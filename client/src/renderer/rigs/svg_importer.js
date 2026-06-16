import {
  RIG_SCHEMA_VERSION,
  TINT_SLOTS,
  validateRigDefinition,
} from "./schema.js";

const ALLOWED_ELEMENTS = new Set(["svg", "g", "path", "polygon", "polyline", "rect", "circle", "ellipse", "line", "metadata"]);
const UNSUPPORTED_ELEMENTS = new Set(["script", "foreignObject", "style", "filter", "mask", "clipPath", "linearGradient", "radialGradient", "pattern", "image", "use", "animate", "animateTransform", "set"]);
const REQUIRED_ROOT_ATTRS = ["viewBox", "data-rts-rig-kind", "data-rts-rig-version", "data-rts-origin"];
const PART_PREFIX = "part.";
const ANCHOR_PREFIX = "anchor.";
const BOUNDS_PREFIX = "bounds.";
const TINT_ATTR = "data-rts-tint";
const XMLNS_ATTRS = new Set(["xmlns", "xmlns:xlink"]);
const PASS_THROUGH_ATTRS = new Set(["id", "transform", "fill", "stroke", "stroke-width", "opacity", "data-rts-pivot", "data-rts-animation", TINT_ATTR]);
const GEOMETRY_ATTRS = new Map(Object.entries({
  path: new Set(["d"]),
  polygon: new Set(["points"]),
  polyline: new Set(["points"]),
  rect: new Set(["x", "y", "width", "height", "rx", "ry"]),
  circle: new Set(["cx", "cy", "r"]),
  ellipse: new Set(["cx", "cy", "rx", "ry"]),
  line: new Set(["x1", "y1", "x2", "y2", "stroke-width"]),
}));
const ROOT_ATTRS = new Set([...REQUIRED_ROOT_ATTRS, "id", "width", "height"]);

export function compileSvgRig(svgText, metadata = {}) {
  const errors = [];
  const document = parseSvgDocument(svgText, errors);
  if (!document) return failed(errors);

  validateElementSubset(document, errors);
  const root = document.name === "svg" ? document : null;
  if (!root) {
    errors.push(error("svg.invalidRoot", "", "Root element must be <svg>."));
    return failed(errors);
  }

  const rootAttrs = root.attrs;
  for (const attr of REQUIRED_ROOT_ATTRS) {
    if (!rootAttrs[attr]) errors.push(error("svg.missingRootAttribute", attr, `Root <svg> is missing ${attr}.`));
  }
  if (rootAttrs["data-rts-origin"] && rootAttrs["data-rts-origin"] !== "center") {
    errors.push(error("svg.unsupportedOrigin", "data-rts-origin", "Only data-rts-origin=\"center\" is supported."));
  }
  if (rootAttrs["data-rts-rig-version"] && rootAttrs["data-rts-rig-version"] !== String(RIG_SCHEMA_VERSION)) {
    errors.push(error("svg.unsupportedRigVersion", "data-rts-rig-version", `Only SVG rig version ${RIG_SCHEMA_VERSION} is supported.`));
  }

  const kind = metadata.kind ?? rootAttrs["data-rts-rig-kind"];
  const id = metadata.id ?? rootAttrs.id ?? `${kind}.svg`;
  const expectedKind = metadata.expectedKind ?? metadata.kind;
  const collected = {
    parts: [],
    anchors: {},
    bounds: {},
    animations: [],
    requiredRuntimeInputs: [],
    drawOrder: 0,
    ids: new Set(),
  };

  collectRigNodes(root, identity(), null, collected, errors);
  if (errors.length > 0) return failed(errors);

  const definition = {
    id,
    kind,
    authoredKind: rootAttrs["data-rts-rig-kind"],
    schemaVersion: RIG_SCHEMA_VERSION,
    parts: collected.parts,
    anchors: collected.anchors,
    bounds: collected.bounds,
    animations: collected.animations,
    requiredRuntimeInputs: collected.requiredRuntimeInputs,
  };
  const validation = validateRigDefinition(definition, expectedKind ? { expectedKind } : {});
  if (!validation.ok) return failed([...errors, ...validation.errors]);
  return { ok: true, definition: validation.definition, errors: [] };
}

function collectRigNodes(node, parentMatrix, inheritedPart, collected, errors) {
  const ownMatrix = multiply(parentMatrix, parseTransform(node.attrs.transform, `#${node.attrs.id ?? node.name}.transform`, errors));
  const id = node.attrs.id ?? "";
  const isPart = id.startsWith(PART_PREFIX);
  const activePart = isPart ? { id, matrix: ownMatrix, attrs: node.attrs } : inheritedPart;

  if (id) {
    if (collected.ids.has(id)) {
      errors.push(error("svg.duplicateId", id, `Duplicate SVG id ${JSON.stringify(id)}.`));
    }
    collected.ids.add(id);
  }

  if (id.startsWith(ANCHOR_PREFIX)) {
    const point = readPointFromNode(node, ownMatrix, id, errors);
    if (point) collected.anchors[id.slice(ANCHOR_PREFIX.length)] = point;
    return;
  }
  if (id.startsWith(BOUNDS_PREFIX)) {
    const bounds = readBoundsFromNode(node, ownMatrix, id, errors);
    if (bounds) collected.bounds[id.slice(BOUNDS_PREFIX.length)] = bounds;
    return;
  }

  if (activePart && isGeometryElement(node.name)) {
    const geometry = readGeometry(node, errors);
    const transform = decompose(ownMatrix, id || activePart.id, errors);
    const pivot = parsePivot(activePart.attrs["data-rts-pivot"], activePart.id, errors);
    const tintSlot = activePart.attrs[TINT_ATTR] ?? node.attrs[TINT_ATTR] ?? "fixed";
    const paint = parsePaint({ ...activePart.attrs, ...node.attrs }, activePart.id, errors);
    if (geometry && transform && pivot && paint && TINT_SLOTS.includes(tintSlot)) {
      collected.parts.push({
        id: activePart.id,
        drawOrder: collected.drawOrder++,
        geometry,
        transform,
        pivot,
        tintSlot,
        paint,
      });
      collectAnimations(activePart.id, activePart.attrs["data-rts-animation"] ?? node.attrs["data-rts-animation"], collected, errors);
    } else if (!TINT_SLOTS.includes(tintSlot)) {
      errors.push(error("svg.invalidTintSlot", `${activePart.id}.${TINT_ATTR}`, `Unsupported tint slot ${JSON.stringify(tintSlot)}.`));
    }
    return;
  }

  for (const child of node.children) {
    collectRigNodes(child, ownMatrix, activePart, collected, errors);
  }
}

function readGeometry(node, errors) {
  if (node.name === "rect") {
    return {
      type: "rect",
      x: readNumberAttr(node, "x", 0, errors),
      y: readNumberAttr(node, "y", 0, errors),
      width: readNumberAttr(node, "width", undefined, errors),
      height: readNumberAttr(node, "height", undefined, errors),
    };
  }
  if (node.name === "circle") {
    return {
      type: "circle",
      cx: readNumberAttr(node, "cx", 0, errors),
      cy: readNumberAttr(node, "cy", 0, errors),
      r: readNumberAttr(node, "r", undefined, errors),
    };
  }
  if (node.name === "ellipse") {
    return {
      type: "ellipse",
      cx: readNumberAttr(node, "cx", 0, errors),
      cy: readNumberAttr(node, "cy", 0, errors),
      rx: readNumberAttr(node, "rx", undefined, errors),
      ry: readNumberAttr(node, "ry", undefined, errors),
    };
  }
  if (node.name === "line") {
    return {
      type: "line",
      from: { x: readNumberAttr(node, "x1", 0, errors), y: readNumberAttr(node, "y1", 0, errors) },
      to: { x: readNumberAttr(node, "x2", 0, errors), y: readNumberAttr(node, "y2", 0, errors) },
      strokeWidth: readNumberAttr(node, "stroke-width", 1, errors),
    };
  }
  if (node.name === "polygon" || node.name === "polyline") {
    return { type: node.name, points: parsePoints(node.attrs.points, node.attrs.id ?? node.name, errors) };
  }
  return { type: "path", commands: parsePathCommands(node.attrs.d, node.attrs.id ?? node.name, errors) };
}

function readPointFromNode(node, matrix, path, errors) {
  let point = null;
  if (node.name === "circle" || node.name === "ellipse") {
    point = { x: readNumberAttr(node, "cx", 0, errors), y: readNumberAttr(node, "cy", 0, errors) };
  } else if (node.name === "rect") {
    const x = readNumberAttr(node, "x", 0, errors);
    const y = readNumberAttr(node, "y", 0, errors);
    const width = readNumberAttr(node, "width", undefined, errors);
    const height = readNumberAttr(node, "height", undefined, errors);
    point = { x: x + width / 2, y: y + height / 2 };
  } else {
    errors.push(error("svg.invalidAnchorElement", path, "Anchors must be circle, ellipse, or rect elements."));
  }
  return point ? roundPoint(apply(matrix, point)) : null;
}

function readBoundsFromNode(node, matrix, path, errors) {
  if (node.name !== "rect") {
    errors.push(error("svg.invalidBoundsElement", path, "Bounds must be rect elements."));
    return null;
  }
  const x = readNumberAttr(node, "x", 0, errors);
  const y = readNumberAttr(node, "y", 0, errors);
  const width = readNumberAttr(node, "width", undefined, errors);
  const height = readNumberAttr(node, "height", undefined, errors);
  const corners = [
    apply(matrix, { x, y }),
    apply(matrix, { x: x + width, y }),
    apply(matrix, { x, y: y + height }),
    apply(matrix, { x: x + width, y: y + height }),
  ];
  const xs = corners.map((point) => point.x);
  const ys = corners.map((point) => point.y);
  const minX = Math.min(...xs);
  const minY = Math.min(...ys);
  return roundRect({ x: minX, y: minY, width: Math.max(...xs) - minX, height: Math.max(...ys) - minY });
}

function collectAnimations(partId, value, collected, errors) {
  if (!value) return;
  for (const entry of value.split(";").map((item) => item.trim()).filter(Boolean)) {
    const [input, property, factorText = "1", offsetText = "0"] = entry.split(":").map((item) => item.trim());
    const factor = Number(factorText);
    const offset = Number(offsetText);
    if (!input || !property || !Number.isFinite(factor) || !Number.isFinite(offset)) {
      errors.push(error("svg.invalidAnimationBinding", partId, `Invalid animation binding ${JSON.stringify(entry)}.`));
      continue;
    }
    collected.animations.push({ partId, input, property, factor, offset });
    if (!collected.requiredRuntimeInputs.includes(input)) collected.requiredRuntimeInputs.push(input);
  }
}

function parsePaint(attrs, path, errors) {
  const fill = parseColorAttr(attrs.fill, `${path}.fill`, errors);
  const stroke = parseColorAttr(attrs.stroke, `${path}.stroke`, errors);
  const strokeWidth = attrs["stroke-width"] == null ? null : parseFinite(attrs["stroke-width"], `${path}.stroke-width`, errors);
  const opacity = attrs.opacity == null ? 1 : parseFinite(attrs.opacity, `${path}.opacity`, errors);
  if (Number.isFinite(opacity) && (opacity < 0 || opacity > 1)) {
    errors.push(error("svg.invalidOpacity", `${path}.opacity`, "Opacity must be between zero and one."));
  }
  return { fill, stroke, strokeWidth, opacity };
}

function parseColorAttr(value, path, errors) {
  if (value == null || value === "none") return null;
  if (/^#[0-9a-fA-F]{6}$/.test(value)) return value.toLowerCase();
  errors.push(error("svg.invalidPaintColor", path, "Only six-digit hex colors and none are supported."));
  return undefined;
}

function validateElementSubset(node, errors) {
  const name = node.name;
  if (UNSUPPORTED_ELEMENTS.has(name)) {
    errors.push(error("svg.unsupportedElement", node.attrs.id ?? name, `<${name}> is not supported in RTS rig SVGs.`));
  } else if (!ALLOWED_ELEMENTS.has(name)) {
    errors.push(error("svg.unsupportedElement", node.attrs.id ?? name, `<${name}> is not in the supported SVG subset.`));
  }
  validateAttributes(node, errors);
  for (const child of node.children) validateElementSubset(child, errors);
}

function validateAttributes(node, errors) {
  const allowed = new Set(PASS_THROUGH_ATTRS);
  if (node.name === "svg") {
    for (const attr of ROOT_ATTRS) allowed.add(attr);
  }
  for (const attr of GEOMETRY_ATTRS.get(node.name) ?? []) allowed.add(attr);

  for (const [name, value] of Object.entries(node.attrs)) {
    if (XMLNS_ATTRS.has(name)) continue;
    if (name === "class" || name === "style") {
      errors.push(error("svg.unsupportedCss", node.attrs.id ?? name, "CSS classes and style attributes are not supported."));
      continue;
    }
    if (name === "href" || name === "xlink:href" || /url\s*\(/i.test(value)) {
      errors.push(error("svg.externalReference", node.attrs.id ?? name, "External references and url() paint servers are not supported."));
      continue;
    }
    if (/%/.test(value)) {
      errors.push(error("svg.percentageUnit", node.attrs.id ?? name, "Percentage units are not supported."));
      continue;
    }
    if (!allowed.has(name) && !name.startsWith("data-rts-")) {
      errors.push(error("svg.unsupportedAttribute", `${node.attrs.id ?? node.name}.${name}`, `Attribute ${JSON.stringify(name)} is not supported.`));
    }
  }
}

function parseSvgDocument(svgText, errors) {
  if (typeof svgText !== "string" || svgText.trim() === "") {
    errors.push(error("svg.invalidInput", "", "SVG input must be non-empty text."));
    return null;
  }
  if (/<!doctype/i.test(svgText)) {
    errors.push(error("svg.unsupportedDoctype", "", "DOCTYPE is not supported."));
    return null;
  }

  const stack = [];
  let root = null;
  const source = svgText.replace(/<\?xml[\s\S]*?\?>/g, "").replace(/<!--[\s\S]*?-->/g, "");
  const tagRe = /<([^>]+)>/g;
  let cursor = 0;
  for (const match of source.matchAll(tagRe)) {
    const text = source.slice(cursor, match.index);
    if (text.trim() && stack.at(-1)?.name !== "metadata") {
      errors.push(error("svg.unexpectedText", stack.at(-1)?.name ?? "", "Unexpected non-whitespace text in SVG."));
    }
    cursor = match.index + match[0].length;

    const raw = match[1].trim();
    if (!raw || raw.startsWith("!")) continue;
    if (raw.startsWith("/")) {
      const name = raw.slice(1).trim();
      const open = stack.pop();
      if (!open || open.name !== name) {
        errors.push(error("svg.mismatchedTag", name, `Unexpected closing tag </${name}>.`));
        return root;
      }
      continue;
    }

    const selfClosing = raw.endsWith("/");
    const body = selfClosing ? raw.slice(0, -1).trim() : raw;
    const space = body.search(/\s/);
    const name = space === -1 ? body : body.slice(0, space);
    const attrText = space === -1 ? "" : body.slice(space + 1);
    const node = { name, attrs: parseAttributes(attrText, name, errors), children: [] };
    if (stack.length === 0) {
      if (root) errors.push(error("svg.multipleRoots", name, "SVG text must contain one root element."));
      root = node;
    } else {
      stack.at(-1).children.push(node);
    }
    if (!selfClosing) stack.push(node);
  }
  if (source.slice(cursor).trim()) errors.push(error("svg.unexpectedText", "", "Unexpected trailing text in SVG."));
  if (stack.length > 0) errors.push(error("svg.unclosedTag", stack.at(-1).name, `Unclosed <${stack.at(-1).name}> tag.`));
  return root;
}

function parseAttributes(text, elementName, errors) {
  const attrs = {};
  const attrRe = /([A-Za-z_:][\w:.-]*)\s*=\s*("([^"]*)"|'([^']*)')/g;
  let cursor = 0;
  for (const match of text.matchAll(attrRe)) {
    if (text.slice(cursor, match.index).trim()) {
      errors.push(error("svg.invalidAttributeSyntax", elementName, `Invalid attribute syntax near ${JSON.stringify(text.slice(cursor, match.index).trim())}.`));
    }
    cursor = match.index + match[0].length;
    const name = match[1];
    if (Object.hasOwn(attrs, name)) {
      errors.push(error("svg.duplicateAttribute", `${elementName}.${name}`, `Duplicate attribute ${JSON.stringify(name)}.`));
    }
    attrs[name] = decodeEntities(match[3] ?? match[4] ?? "");
  }
  if (text.slice(cursor).trim()) {
    errors.push(error("svg.invalidAttributeSyntax", elementName, `Invalid attribute syntax near ${JSON.stringify(text.slice(cursor).trim())}.`));
  }
  return attrs;
}

function decodeEntities(value) {
  return value
    .replaceAll("&quot;", "\"")
    .replaceAll("&apos;", "'")
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&amp;", "&");
}

function parseTransform(value, path, errors) {
  if (!value) return identity();
  let matrix = identity();
  const re = /(translate|rotate|scale|matrix)\(([^)]*)\)/g;
  let cursor = 0;
  for (const match of value.matchAll(re)) {
    if (value.slice(cursor, match.index).trim()) {
      errors.push(error("svg.unsupportedTransform", path, `Unsupported transform syntax ${JSON.stringify(value)}.`));
      return identity();
    }
    cursor = match.index + match[0].length;
    const values = parseNumberList(match[2], path, errors);
    matrix = multiply(matrix, transformMatrix(match[1], values, path, errors));
  }
  if (value.slice(cursor).trim()) errors.push(error("svg.unsupportedTransform", path, `Unsupported transform syntax ${JSON.stringify(value)}.`));
  return matrix;
}

function transformMatrix(name, values, path, errors) {
  if (name === "translate") return [1, 0, 0, 1, values[0] ?? 0, values[1] ?? 0];
  if (name === "scale") return [values[0] ?? 1, 0, 0, values[1] ?? values[0] ?? 1, 0, 0];
  if (name === "rotate") {
    if (values.length > 1) {
      errors.push(error("svg.unsupportedTransform", path, "Rotate about an arbitrary pivot is not supported; use data-rts-pivot."));
      return identity();
    }
    const radians = ((values[0] ?? 0) * Math.PI) / 180;
    const cos = Math.cos(radians);
    const sin = Math.sin(radians);
    return [cos, sin, -sin, cos, 0, 0];
  }
  if (values.length !== 6) {
    errors.push(error("svg.unsupportedTransform", path, "Matrix transforms must include six finite values."));
    return identity();
  }
  return values;
}

function decompose(matrix, path, errors) {
  const [a, b, c, d, e, f] = matrix;
  const scaleX = Math.hypot(a, b);
  if (scaleX === 0) {
    errors.push(error("svg.unsupportedTransform", path, "Transforms with zero scale are not supported."));
    return null;
  }
  const rotation = Math.atan2(b, a);
  const det = a * d - b * c;
  const scaleY = det / scaleX;
  const shear = a * c + b * d;
  if (Math.abs(shear) > 1e-6) {
    errors.push(error("svg.unsupportedTransform", path, "Skewed or non-decomposable matrix transforms are not supported."));
    return null;
  }
  return roundTransform({ x: e, y: f, rotation, scaleX, scaleY });
}

function parsePivot(value, path, errors) {
  if (!value) return { x: 0, y: 0 };
  const values = parseNumberList(value, `${path}.data-rts-pivot`, errors);
  if (values.length !== 2) {
    errors.push(error("svg.invalidPivot", `${path}.data-rts-pivot`, "Pivots must be two finite numbers."));
    return null;
  }
  return roundPoint({ x: values[0], y: values[1] });
}

function parsePoints(value, path, errors) {
  const values = parseNumberList(value, `${path}.points`, errors);
  if (values.length % 2 !== 0) errors.push(error("svg.invalidPoints", `${path}.points`, "Points must contain x/y pairs."));
  const out = [];
  for (let index = 0; index + 1 < values.length; index += 2) {
    out.push(roundPoint({ x: values[index], y: values[index + 1] }));
  }
  return out;
}

function parsePathCommands(value, path, errors) {
  if (!value) {
    errors.push(error("svg.invalidPath", `${path}.d`, "Path data is required."));
    return [];
  }
  if (/[a-z]/.test(value)) {
    errors.push(error("svg.unsupportedPathCommand", `${path}.d`, "Only absolute SVG path commands are supported."));
    return [];
  }
  const unsupported = value.match(/[A-Z]/g)?.find((command) => !["M", "L", "C", "Q", "Z"].includes(command));
  if (unsupported) {
    errors.push(error("svg.unsupportedPathCommand", `${path}.d`, `Unsupported path command ${JSON.stringify(unsupported)}.`));
    return [];
  }
  const tokens = value.match(/[MLCQZ]|[-+]?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?/g) ?? [];
  const commands = [];
  let index = 0;
  while (index < tokens.length) {
    const command = tokens[index++];
    const count = { M: 2, L: 2, C: 6, Q: 4, Z: 0 }[command];
    if (count == null) {
      errors.push(error("svg.unsupportedPathCommand", `${path}.d`, `Unsupported path command ${JSON.stringify(command)}.`));
      break;
    }
    const values = [];
    for (let i = 0; i < count; i += 1) {
      const number = Number(tokens[index++]);
      if (!Number.isFinite(number)) errors.push(error("svg.invalidPath", `${path}.d`, "Path command has missing or non-finite values."));
      values.push(round(number));
    }
    commands.push({ command, values });
  }
  return commands;
}

function parseNumberList(value, path, errors) {
  if (typeof value !== "string" || value.trim() === "") return [];
  const chunks = value.trim().split(/[\s,]+/).filter(Boolean);
  const numbers = [];
  for (const chunk of chunks) {
    const number = parseFinite(chunk, path, errors);
    if (Number.isFinite(number)) numbers.push(number);
  }
  return numbers;
}

function readNumberAttr(node, attr, fallback, errors) {
  if (node.attrs[attr] == null) {
    if (fallback !== undefined) return fallback;
    errors.push(error("svg.missingNumber", `${node.attrs.id ?? node.name}.${attr}`, `Missing numeric attribute ${attr}.`));
    return NaN;
  }
  return parseFinite(node.attrs[attr], `${node.attrs.id ?? node.name}.${attr}`, errors);
}

function parseFinite(value, path, errors) {
  const number = Number(value);
  if (!Number.isFinite(number)) {
    errors.push(error("svg.nonFiniteNumber", path, `${path} must be a finite number.`));
    return NaN;
  }
  return number;
}

function isGeometryElement(name) {
  return GEOMETRY_ATTRS.has(name);
}

function identity() {
  return [1, 0, 0, 1, 0, 0];
}

function multiply(left, right) {
  return [
    left[0] * right[0] + left[2] * right[1],
    left[1] * right[0] + left[3] * right[1],
    left[0] * right[2] + left[2] * right[3],
    left[1] * right[2] + left[3] * right[3],
    left[0] * right[4] + left[2] * right[5] + left[4],
    left[1] * right[4] + left[3] * right[5] + left[5],
  ];
}

function apply(matrix, point) {
  return {
    x: matrix[0] * point.x + matrix[2] * point.y + matrix[4],
    y: matrix[1] * point.x + matrix[3] * point.y + matrix[5],
  };
}

function roundTransform(transform) {
  return {
    x: round(transform.x),
    y: round(transform.y),
    rotation: round(transform.rotation),
    scaleX: round(transform.scaleX),
    scaleY: round(transform.scaleY),
  };
}

function roundPoint(point) {
  return { x: round(point.x), y: round(point.y) };
}

function roundRect(rect) {
  return { x: round(rect.x), y: round(rect.y), width: round(rect.width), height: round(rect.height) };
}

function round(value) {
  return Math.round(value * 1_000_000) / 1_000_000;
}

function failed(errors) {
  return { ok: false, errors };
}

function error(code, path, message) {
  return { code, path, message };
}
