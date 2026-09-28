export function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);

  if (className) {
    node.className = className;
  }

  if (text !== undefined) {
    node.textContent = text;
  }

  return node;
}

export function setAria(element: HTMLElement, attributes: Record<string, string>): void {
  for (const [name, value] of Object.entries(attributes)) {
    element.setAttribute(`aria-${name}`, value);
  }
}
