/**
 * @file src/ui/safeDom.ts
 * @description Safe DOM rendering helpers following GoogleChrome/modern-web-guidance
 * (`sanitize-untrusted-html`).
 *
 * - Prefer structured DOM creation (`createDomElement()`, `textContent`, `replaceChildren()`)
 *   for all plain-text UI fields so dynamic strings are never parsed as HTML.
 * - When rich inline HTML formatting is explicitly needed, `setSanitizedHTML()` uses the
 *   native browser Sanitizer API (`Element.prototype.setHTML`) with an explicit allowlist,
 *   falling back to `<template>` allowlist DOM node filtering without serializing back to HTML.
 */

export interface AllowedHtmlConfig {
  elements: readonly string[];
  attributes: readonly string[];
}

interface SanitizerCapableElement extends Element {
  setHTML?: (
    input: string,
    options?: { sanitizer?: { elements?: string[]; attributes?: string[] } }
  ) => void;
}

const DEFAULT_INLINE_CONFIG: AllowedHtmlConfig = {
  elements: ['p', 'b', 'i', 'strong', 'em', 'span', 'br', 'blockquote'],
  attributes: ['class'],
};

/**
 * Safely parses and inserts restricted HTML into `container` using native
 * `Element.prototype.setHTML()` when supported, or a strict `<template>` allowlist
 * walker that strips disallowed elements (along with their subtrees), scripts, and
 * `on*`/`aria-*`/`data-*` attributes before moving sanitized DOM nodes via `replaceChildren()`.
 */
export function setSanitizedHTML(
  container: Element,
  html: string,
  config: AllowedHtmlConfig = DEFAULT_INLINE_CONFIG
): void {
  const elements = [...config.elements];
  const attributes = [...config.attributes];

  if (
    typeof Element !== 'undefined' &&
    'setHTML' in Element.prototype &&
    typeof (container as SanitizerCapableElement).setHTML === 'function'
  ) {
    (container as SanitizerCapableElement).setHTML!(html, {
      sanitizer: { elements, attributes },
    });
    return;
  }

  const doc = container.ownerDocument ?? document;
  const template = doc.createElement('template');
  template.innerHTML = html;

  const allowedElements = new Set(elements.map((tag) => tag.toLowerCase()));
  const allowedAttributes = new Set(attributes.map((attr) => attr.toLowerCase()));

  const sanitizeNode = (node: Node): Node | null => {
    if (node.nodeType === Node.TEXT_NODE) {
      return doc.createTextNode(node.textContent ?? '');
    }
    if (node.nodeType !== Node.ELEMENT_NODE) {
      return null;
    }

    const srcEl = node as Element;
    const tag = srcEl.tagName.toLowerCase();
    // Native Sanitizer removes disallowed elements together with their content
    if (!allowedElements.has(tag)) {
      return null;
    }

    const cleanEl = doc.createElement(tag);
    for (const attr of Array.from(srcEl.attributes)) {
      const name = attr.name.toLowerCase();
      if (allowedAttributes.has(name) && !name.startsWith('on')) {
        cleanEl.setAttribute(attr.name, attr.value);
      }
    }

    for (const child of Array.from(srcEl.childNodes)) {
      const cleanChild = sanitizeNode(child);
      if (cleanChild) {
        cleanEl.appendChild(cleanChild);
      }
    }
    return cleanEl;
  };

  const fragment = doc.createDocumentFragment();
  for (const child of Array.from(template.content.childNodes)) {
    const cleanChild = sanitizeNode(child);
    if (cleanChild) {
      fragment.appendChild(cleanChild);
    }
  }
  container.replaceChildren(fragment);
}

/**
 * Creates an HTMLElement with optional className, textContent, and attributes
 * without invoking an HTML parser.
 */
export function createDomElement<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  options?: {
    className?: string;
    text?: string;
    attrs?: Record<string, string>;
  },
  children: Array<Node | null | undefined> = []
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  if (options?.className) {
    element.className = options.className;
  }
  if (options?.text !== undefined) {
    element.textContent = options.text;
  }
  if (options?.attrs) {
    for (const [key, value] of Object.entries(options.attrs)) {
      element.setAttribute(key, value);
    }
  }
  for (const child of children) {
    if (child) {
      element.appendChild(child);
    }
  }
  return element;
}
