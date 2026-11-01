  // Keys: a chord such as "ArrowUp", "Enter", "Ctrl+A" or "a" becomes
  // keydown (and keypress for a character) then keyup on the focused element, carrying `key`,
  // `code` and the legacy `keyCode`/`which` that games read. Synthetic keys run no default action,
  // so they are done here: Tab moves focus, Enter submits a form, and in a field a character is
  // typed at the caret, Backspace and Delete erase, Ctrl+A selects all (as `input` events the
  // page hears), unless the page's own handler cancelled the key.
  const NAMED_KEYS = {
    enter: 'Enter', return: 'Enter', tab: 'Tab', escape: 'Escape', esc: 'Escape', space: ' ',
    spacebar: ' ', backspace: 'Backspace', delete: 'Delete', del: 'Delete', arrowup: 'ArrowUp',
    up: 'ArrowUp', arrowdown: 'ArrowDown', down: 'ArrowDown', arrowleft: 'ArrowLeft',
    left: 'ArrowLeft', arrowright: 'ArrowRight', right: 'ArrowRight', home: 'Home', end: 'End',
    pageup: 'PageUp', pagedown: 'PageDown',
  }
  const KEY_CODES = {
    Enter: 13, Tab: 9, Escape: 27, ' ': 32, Backspace: 8, Delete: 46, ArrowUp: 38, ArrowDown: 40,
    ArrowLeft: 37, ArrowRight: 39, Home: 36, End: 35, PageUp: 33, PageDown: 34,
  }
  const physicalCode = (key) => {
    if (/^[a-z]$/i.test(key)) return `Key${key.toUpperCase()}`
    if (/^[0-9]$/.test(key)) return `Digit${key}`
    if (key === ' ') return 'Space'
    return key.length > 1 ? key : ''
  }

  function chord(text) {
    const parts = String(text).trim().split('+').map((p) => p.trim())
    let key = parts.pop() || (String(text).endsWith('+') ? '+' : '')
    const mods = new Set(parts.map((m) => m.toLowerCase()))
    key = NAMED_KEYS[key.toLowerCase()] ?? key
    return {
      key, code: physicalCode(key), bubbles: true, cancelable: true, composed: true,
      ctrlKey: mods.has('ctrl') || mods.has('control'), metaKey: mods.has('meta') || mods.has('cmd'),
      altKey: mods.has('alt'), shiftKey: mods.has('shift'),
    }
  }

  // A field that takes typing: a text-like input, a textarea, an editable element.
  const TEXT_TYPES = new Set(['', 'text', 'search', 'email', 'url', 'tel', 'password', 'number'])
  const editable = (el) => !!el && !el.disabled && !el.readOnly && (el.isContentEditable
    || el.tagName === 'TEXTAREA' || (el.tagName === 'INPUT' && TEXT_TYPES.has(el.type)))

  // One edit at the caret of a field, the way typing makes it: `beforeinput` (which the page may
  // cancel), the change, then `input`. Done on the field itself, not through the window's
  // selection, so it works while the window is not the focused one.
  function edit(el, inputType, data = null) {
    const before = new InputEvent('beforeinput', { inputType, data, bubbles: true, cancelable: true, composed: true })
    if (!el.dispatchEvent(before)) return
    if (el.isContentEditable) {
      const sel = getSelection()
      if (!sel.rangeCount || !el.contains(sel.anchorNode)) {
        sel.selectAllChildren(el)
        sel.collapseToEnd()
      }
      const range = sel.getRangeAt(0)
      if (range.collapsed && inputType !== 'insertText') {
        sel.modify('extend', inputType === 'deleteContentBackward' ? 'backward' : 'forward', 'character')
      }
      const r = sel.getRangeAt(0)
      r.deleteContents()
      if (data) {
        const node = document.createTextNode(data)
        r.insertNode(node)
        sel.collapse(node, node.length)
      }
    } else {
      // A number input has no selection range; it is edited at its end.
      let start = el.value.length
      let end = start
      try {
        if (el.selectionStart != null) { start = el.selectionStart; end = el.selectionEnd }
      } catch {}
      if (start === end && inputType === 'deleteContentBackward') start = Math.max(0, start - 1)
      if (start === end && inputType === 'deleteContentForward') end = Math.min(el.value.length, end + 1)
      const value = el.value.slice(0, start) + (data ?? '') + el.value.slice(end)
      // Through the element's own setter, which a framework's controlled field watches.
      const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), 'value')?.set
      if (setter) setter.call(el, value)
      else el.value = value
      const caret = start + (data ?? '').length
      try { el.setSelectionRange(caret, caret) } catch {}
    }
    el.dispatchEvent(new InputEvent('input', { inputType, data, bubbles: true, composed: true }))
  }

  function press(text) {
    const init = chord(text)
    const legacy = KEY_CODES[init.key] ?? (init.key.length === 1 ? init.key.toUpperCase().charCodeAt(0) : 0)
    const target = document.activeElement && document.activeElement !== document.documentElement
      ? document.activeElement : document.body
    const fire = (type) => {
      const event = new KeyboardEvent(type, init)
      Object.defineProperty(event, 'keyCode', { get: () => legacy })
      Object.defineProperty(event, 'which', { get: () => legacy })
      return target.dispatchEvent(event)
    }
    const proceed = fire('keydown')
    const character = init.key.length === 1 && !init.ctrlKey && !init.metaKey
    const typed = proceed && character && fire('keypress')
    if (proceed && editable(target)) {
      if (typed && !init.altKey) edit(target, 'insertText', init.key)
      else if (init.key === 'Backspace') edit(target, 'deleteContentBackward')
      else if (init.key === 'Delete') edit(target, 'deleteContentForward')
      else if ((init.ctrlKey || init.metaKey) && init.key.toLowerCase() === 'a') {
        if (target.select) target.select()
        else getSelection().selectAllChildren(target)
      }
    }
    fire('keyup')
    if (proceed && init.key === 'Enter' && target.form) target.form.requestSubmit()
    if (proceed && init.key === 'Tab') {
      const order = [...document.querySelectorAll('a[href],button,input,select,textarea,[tabindex]')]
        .filter((e) => e.tabIndex >= 0 && !e.disabled && visible(e))
      const at = order.indexOf(target)
      order[at + (init.shiftKey ? -1 : 1)]?.focus()
    }
  }

  // A sequence of keys, `delay` ms apart so a game's animation can take each one.
  async function keys(args) {
    begin()
    const list = (args.keys || []).slice(0, 500)
    for (const [i, key] of list.entries()) {
      press(key)
      if (i < list.length - 1) await new Promise((r) => setTimeout(r, args.delay ?? 60))
    }
    return { pressed: list.length }
  }

