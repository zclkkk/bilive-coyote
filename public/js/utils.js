export function escapeHtml(str) {
  const el = document.createElement("span")
  el.textContent = str
  return el.innerHTML
}
