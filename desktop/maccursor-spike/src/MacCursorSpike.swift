#if os(macOS)
import AppKit
import CoreGraphics
import Darwin

private let markerDiameter: CGFloat = 18.0
private let defaultWindowSize = NSSize(width: 920, height: 560)
private var emergencyCursorHidden = false
private var emergencyCursorDisconnected = false

private struct CursorSnapshot {
    var active: Bool
    var position: CGPoint
    var eventCount: UInt64
    var lastLatencyMilliseconds: Double
    var lastReason: String
    var lastError: String?
}

private final class MarkerView: NSView {
    private var snapshot: CursorSnapshot

    init(frame frameRect: NSRect, initialPosition: CGPoint) {
        snapshot = CursorSnapshot(
            active: false,
            position: initialPosition,
            eventCount: 0,
            lastLatencyMilliseconds: 0.0,
            lastReason: "launching",
            lastError: nil
        )
        super.init(frame: frameRect)
        wantsLayer = true
    }

    required init?(coder: NSCoder) {
        fatalError("MacCursorSpike is not loaded from a nib")
    }

    override var acceptsFirstResponder: Bool {
        true
    }

    override var isFlipped: Bool {
        true
    }

    func update(_ snapshot: CursorSnapshot) {
        self.snapshot = snapshot
        display()
    }

    override func draw(_ dirtyRect: NSRect) {
        NSColor(calibratedRed: 0.06, green: 0.07, blue: 0.08, alpha: 1.0).setFill()
        bounds.fill()

        drawStatus()
        drawMarker()
    }

    private func drawStatus() {
        let status = snapshot.active ? "capture active" : "capture released"
        let errorLine = snapshot.lastError.map { "error \($0)" } ?? "error none"
        let text = """
        \(status)
        events \(snapshot.eventCount)
        event-to-handler \(String(format: "%.3f", snapshot.lastLatencyMilliseconds)) ms
        reason \(snapshot.lastReason)
        \(errorLine)
        Escape releases; Space captures
        """
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineSpacing = 3.0
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 13.0, weight: .medium),
            .foregroundColor: NSColor(calibratedWhite: 0.92, alpha: 1.0),
            .paragraphStyle: paragraph,
        ]
        text.draw(in: NSRect(x: 18, y: 18, width: 420, height: 150), withAttributes: attrs)
    }

    private func drawMarker() {
        let rect = NSRect(
            x: snapshot.position.x - markerDiameter / 2.0,
            y: snapshot.position.y - markerDiameter / 2.0,
            width: markerDiameter,
            height: markerDiameter
        )
        let outer = NSBezierPath(ovalIn: rect)
        NSColor.systemYellow.setFill()
        outer.fill()

        NSColor.black.setStroke()
        outer.lineWidth = 2.0
        outer.stroke()

        let crosshair = NSBezierPath()
        crosshair.move(to: CGPoint(x: rect.midX - 13, y: rect.midY))
        crosshair.line(to: CGPoint(x: rect.midX + 13, y: rect.midY))
        crosshair.move(to: CGPoint(x: rect.midX, y: rect.midY - 13))
        crosshair.line(to: CGPoint(x: rect.midX, y: rect.midY + 13))
        NSColor.systemYellow.setStroke()
        crosshair.lineWidth = 1.0
        crosshair.stroke()
    }
}

private final class NativeCaptureSession {
    private weak var markerView: MarkerView?
    private let boundsProvider: () -> CGRect
    private let controlsSystemCursor: Bool
    private var snapshot: CursorSnapshot
    private var cursorHidden = false
    private var cursorDisconnected = false

    init(
        markerView: MarkerView?,
        initialPosition: CGPoint,
        boundsProvider: @escaping () -> CGRect,
        controlsSystemCursor: Bool = true
    ) {
        self.markerView = markerView
        self.boundsProvider = boundsProvider
        self.controlsSystemCursor = controlsSystemCursor
        snapshot = CursorSnapshot(
            active: false,
            position: initialPosition,
            eventCount: 0,
            lastLatencyMilliseconds: 0.0,
            lastReason: "ready",
            lastError: nil
        )
        markerView?.update(snapshot)
    }

    var isActive: Bool {
        snapshot.active
    }

    var position: CGPoint {
        snapshot.position
    }

    func start(reason: String) -> Bool {
        guard !snapshot.active else {
            return true
        }

        if !controlsSystemCursor {
            snapshot.active = true
            snapshot.lastReason = reason
            snapshot.lastError = nil
            markerView?.update(snapshot)
            print("maccursor-spike capture simulated reason=\(reason)")
            return true
        }

        let display = CGMainDisplayID()
        let hideResult = CGDisplayHideCursor(display)
        let associateResult = CGAssociateMouseAndMouseCursorPosition(boolean_t(0))

        cursorHidden = hideResult == .success
        cursorDisconnected = associateResult == .success
        emergencyCursorHidden = cursorHidden
        emergencyCursorDisconnected = cursorDisconnected
        snapshot.active = cursorHidden && cursorDisconnected
        snapshot.lastReason = reason
        snapshot.lastError = combinedError(hideResult: hideResult, associateResult: associateResult)
        markerView?.update(snapshot)

        if snapshot.active {
            print("maccursor-spike capture started reason=\(reason)")
        } else {
            restore(reason: "partial start failure")
            print(
                "maccursor-spike capture failed hide=\(hideResult.rawValue) associate=\(associateResult.rawValue)",
                to: &standardError
            )
        }
        return snapshot.active
    }

    func stop(reason: String) {
        restore(reason: reason)
    }

    func handleMouseEvent(_ event: NSEvent) -> NSEvent? {
        guard snapshot.active else {
            return event
        }

        moveBy(deltaX: CGFloat(event.deltaX), deltaY: CGFloat(event.deltaY), eventTimestamp: event.timestamp)
        return nil
    }

    func moveBy(deltaX: CGFloat, deltaY: CGFloat, eventTimestamp: TimeInterval) {
        let bounds = boundsProvider()
        let minX = bounds.minX + markerDiameter / 2.0
        let maxX = bounds.maxX - markerDiameter / 2.0
        let minY = bounds.minY + markerDiameter / 2.0
        let maxY = bounds.maxY - markerDiameter / 2.0

        snapshot.position.x = clamp(snapshot.position.x + deltaX, minX, maxX)
        snapshot.position.y = clamp(snapshot.position.y + deltaY, minY, maxY)
        snapshot.eventCount += 1
        snapshot.lastLatencyMilliseconds = max(
            0.0,
            (ProcessInfo.processInfo.systemUptime - eventTimestamp) * 1000.0
        )
        snapshot.lastReason = "native mouse event"
        snapshot.lastError = nil
        markerView?.update(snapshot)

        if snapshot.eventCount % 120 == 0 {
            print(
                "maccursor-spike events=\(snapshot.eventCount) event_to_handler_ms="
                    + String(format: "%.3f", snapshot.lastLatencyMilliseconds)
            )
        }
    }

    private func restore(reason: String) {
        if cursorDisconnected {
            let result = CGAssociateMouseAndMouseCursorPosition(boolean_t(1))
            if result != .success {
                print("maccursor-spike cursor reassociation failed code=\(result.rawValue)", to: &standardError)
            }
            cursorDisconnected = false
            emergencyCursorDisconnected = false
        }

        if cursorHidden {
            let result = CGDisplayShowCursor(CGMainDisplayID())
            if result != .success {
                print("maccursor-spike cursor show failed code=\(result.rawValue)", to: &standardError)
            }
            cursorHidden = false
            emergencyCursorHidden = false
        }

        let wasActive = snapshot.active
        snapshot.active = false
        snapshot.lastReason = reason
        markerView?.update(snapshot)

        if wasActive {
            print("maccursor-spike capture stopped reason=\(reason)")
        }
    }

    private func combinedError(hideResult: CGError, associateResult: CGError) -> String? {
        var failures: [String] = []
        if hideResult != .success {
            failures.append("hide=\(hideResult.rawValue)")
        }
        if associateResult != .success {
            failures.append("associate=\(associateResult.rawValue)")
        }
        return failures.isEmpty ? nil : failures.joined(separator: " ")
    }
}

private final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    private var window: NSWindow?
    private var markerView: MarkerView?
    private var captureSession: NativeCaptureSession?
    private var mouseMonitor: Any?
    private var keyMonitor: Any?
    private var shouldStartCaptureOnActivation = true

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        buildWindow()
        installEventMonitors()
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        if shouldStartCaptureOnActivation {
            shouldStartCaptureOnActivation = false
            _ = captureSession?.start(reason: "app activated")
        }
    }

    func applicationDidResignActive(_ notification: Notification) {
        captureSession?.stop(reason: "app deactivated")
    }

    func applicationWillTerminate(_ notification: Notification) {
        removeEventMonitors()
        captureSession?.stop(reason: "app terminated")
    }

    func windowWillClose(_ notification: Notification) {
        captureSession?.stop(reason: "window closed")
        NSApp.terminate(nil)
    }

    func windowDidResignKey(_ notification: Notification) {
        captureSession?.stop(reason: "window blur")
    }

    private func buildWindow() {
        let frame = NSRect(origin: .zero, size: defaultWindowSize)
        let view = MarkerView(
            frame: frame,
            initialPosition: CGPoint(x: defaultWindowSize.width / 2.0, y: defaultWindowSize.height / 2.0)
        )
        let session = NativeCaptureSession(
            markerView: view,
            initialPosition: CGPoint(x: defaultWindowSize.width / 2.0, y: defaultWindowSize.height / 2.0),
            boundsProvider: { [weak view] in view?.bounds ?? CGRect(origin: .zero, size: defaultWindowSize) }
        )
        let window = NSWindow(
            contentRect: frame,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )

        window.title = "macOS Native Cursor Capture Spike"
        window.contentView = view
        window.delegate = self
        window.acceptsMouseMovedEvents = true
        window.isReleasedWhenClosed = false
        window.center()
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(view)

        self.window = window
        self.markerView = view
        captureSession = session
    }

    private func installEventMonitors() {
        let mouseMask: NSEvent.EventTypeMask = [
            .mouseMoved,
            .leftMouseDragged,
            .rightMouseDragged,
            .otherMouseDragged,
        ]
        mouseMonitor = NSEvent.addLocalMonitorForEvents(matching: mouseMask) { [weak self] event in
            self?.captureSession?.handleMouseEvent(event) ?? event
        }

        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            if event.keyCode == 53 {
                self?.captureSession?.stop(reason: "escape")
                return nil
            }
            if event.charactersIgnoringModifiers == " " {
                _ = self?.captureSession?.start(reason: "space")
                return nil
            }
            return event
        }
    }

    private func removeEventMonitors() {
        if let mouseMonitor {
            NSEvent.removeMonitor(mouseMonitor)
            self.mouseMonitor = nil
        }
        if let keyMonitor {
            NSEvent.removeMonitor(keyMonitor)
            self.keyMonitor = nil
        }
    }
}

private struct StandardError: TextOutputStream {
    mutating func write(_ string: String) {
        FileHandle.standardError.write(Data(string.utf8))
    }
}

private var standardError = StandardError()

private func clamp<T: Comparable>(_ value: T, _ minValue: T, _ maxValue: T) -> T {
    min(max(value, minValue), maxValue)
}

private func emergencyRestoreAndExit(_ signalNumber: Int32) {
    if emergencyCursorDisconnected {
        _ = CGAssociateMouseAndMouseCursorPosition(boolean_t(1))
    }
    if emergencyCursorHidden {
        _ = CGDisplayShowCursor(CGMainDisplayID())
    }
    _exit(128 + signalNumber)
}

private func installEmergencySignalHandlers() {
    signal(SIGINT, emergencyRestoreAndExit)
    signal(SIGTERM, emergencyRestoreAndExit)
    signal(SIGHUP, emergencyRestoreAndExit)
}

private func runSelfTest() -> Int32 {
    let bounds = CGRect(origin: .zero, size: defaultWindowSize)
    let session = NativeCaptureSession(
        markerView: nil,
        initialPosition: CGPoint(x: bounds.midX, y: bounds.midY),
        boundsProvider: { bounds },
        controlsSystemCursor: false
    )

    guard session.start(reason: "self-test") else {
        return 1
    }
    session.moveBy(
        deltaX: 11.0,
        deltaY: -7.0,
        eventTimestamp: ProcessInfo.processInfo.systemUptime
    )
    guard session.position.x == bounds.midX + 11.0 && session.position.y == bounds.midY - 7.0 else {
        print("maccursor-spike self-test moved to unexpected position \(session.position)", to: &standardError)
        return 1
    }
    session.stop(reason: "self-test complete")

    guard !session.isActive else {
        print("maccursor-spike self-test left capture active", to: &standardError)
        return 1
    }

    print("maccursor-spike self-test passed")
    return 0
}

if CommandLine.arguments.contains("--self-test") {
    exit(runSelfTest())
}

private let app = NSApplication.shared
private let delegate = AppDelegate()
installEmergencySignalHandlers()
app.delegate = delegate
app.run()
#else
print("maccursor-spike requires macOS.")
exit(1)
#endif
