export function textWithin(node) {
  if (!node) return "";
  let out = node.textContent || "";
  for (const child of node.children || []) out += ` ${textWithin(child)}`;
  return out;
}
