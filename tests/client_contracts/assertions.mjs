export function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

export function assertDeepEqual(actual, expected, msg) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), msg);
}

export function assertApprox(actual, expected, epsilon, msg) {
  assert(
    Math.abs(actual - expected) <= epsilon,
    `${msg}: expected ${expected}, got ${actual}`,
  );
}

export function assertThrows(fn, msg) {
  let threw = false;
  try {
    fn();
  } catch (err) {
    threw = true;
  }
  assert(threw, msg);
}

export function assertHasMethod(obj, name, msgPrefix = "") {
  assert(
    typeof obj[name] === "function",
    `${msgPrefix || "Object"} missing method "${name}"`,
  );
}

export function assertHasGetter(obj, name, msgPrefix = "") {
  const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(obj) || obj, name);
  assert(
    d && typeof d.get === "function",
    `${msgPrefix || "Object"} missing getter "${name}"`,
  );
}
