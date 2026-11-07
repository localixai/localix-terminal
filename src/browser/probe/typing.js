  // Typing into a field. The value is set at once through the native setter (so React and Vue
  // value trackers see it), then read back: a field whose script rewrote or refused it (an input
  // mask for a phone, a card or a date, a box that reads key by key) is cleared and typed again a
  // key at a time, as a person types. Letters and digits are compared, so a mask's own spaces,
  // dashes and brackets count as kept. A date field takes its value in ISO form, as the browser
  // does: a date written another way is read into it. A field that keeps something else even then
  // is told (told.js), never with a password's value.
  const comparable = (s) => String(s).toLowerCase().replace(/[^\p{L}\p{N}]/gu, '')
  // A value of signs only (`!!!!`) is compared whole: with no letters or digits, any field matches.
  const same = (a, b) => {
    const x = comparable(a)
    const y = comparable(b)
    return x || y ? x === y : String(a) === String(b)
  }
  // Key by key only up to this many characters: a long text is a paste, as a person pastes it.
  const MAX_KEYED = 200
  const KEY_PAUSE = 12
  // Inputs that are not typed into: clicked, or (a file) not set by a page's script at all.
  const NOT_TEXT = new Set(['checkbox', 'radio', 'button', 'submit', 'reset', 'image', 'file'])

  function setValue(e, value) {
    const view = e.ownerDocument.defaultView
    const proto = e instanceof view.HTMLTextAreaElement ? view.HTMLTextAreaElement : view.HTMLInputElement
    Object.getOwnPropertyDescriptor(proto.prototype, 'value').set.call(e, value)
  }

  async function fill(e, text) {
    const view = e.ownerDocument.defaultView
    e.focus?.({ preventScroll: true })
    const field = e instanceof view.HTMLInputElement || e instanceof view.HTMLTextAreaElement
    // A checkbox or a button takes no text: writing its `value` would report a fill and toggle nothing.
    if (field && NOT_TEXT.has(e.type)) {
      throw new Error(e.type === 'file' ? 'a file field cannot be filled'
        : `this is a ${e.type}, not a field: click it instead of filling it`)
    }
    if (!field && e.isContentEditable) {
      e.textContent = text
      e.dispatchEvent(new view.InputEvent('input', { bubbles: true, inputType: 'insertText', data: text }))
      return
    }
    if (!field) throw new Error('not editable: this is not a text field; fill takes a textbox, a searchbox or a field of the outline')
    const value = e.type === 'date' ? isoDate(text) : text
    setValue(e, value)
    e.dispatchEvent(new view.InputEvent('input', { bubbles: true, inputType: 'insertText', data: value }))
    e.dispatchEvent(new view.Event('change', { bubbles: true }))
    const kept = () => same(e.value, value)
    if (!kept() && value.length <= MAX_KEYED && e.type !== 'date') await keyByKey(e, value)
    if (kept()) return
    const name = nameOf(e) || 'a field'
    tell({ kind: 'field', text: e.type === 'password' ? `${name} did not keep the typed value`
      : `${name} kept "${clip(e.value, 80)}" of the typed "${clip(value, 80)}"` })
  }

  // The field cleared, then each character as a key: keydown, keypress and beforeinput first, so
  // a mask that takes the key itself (and cancels it) does; else the character goes in at the
  // caret, and the input event follows.
  async function keyByKey(e, text) {
    const view = e.ownerDocument.defaultView
    setValue(e, '')
    e.dispatchEvent(new view.InputEvent('input', { bubbles: true, inputType: 'deleteContentBackward' }))
    for (const ch of text) {
      const init = { key: ch, code: physicalCode(ch), bubbles: true, cancelable: true, composed: true }
      const proceed = e.dispatchEvent(new view.KeyboardEvent('keydown', init)) &&
        e.dispatchEvent(new view.KeyboardEvent('keypress', init)) &&
        e.dispatchEvent(new view.InputEvent('beforeinput', { bubbles: true, cancelable: true, inputType: 'insertText', data: ch }))
      if (proceed) {
        let at = null
        try { at = e.selectionStart } catch {}
        if (at === null || at === undefined) at = e.value.length
        setValue(e, e.value.slice(0, at) + ch + e.value.slice(at))
        try { e.setSelectionRange(at + 1, at + 1) } catch {}
        e.dispatchEvent(new view.InputEvent('input', { bubbles: true, inputType: 'insertText', data: ch }))
      }
      e.dispatchEvent(new view.KeyboardEvent('keyup', init))
      await pause(KEY_PAUSE)
    }
    e.dispatchEvent(new view.Event('change', { bubbles: true }))
  }

  // A date as a date field takes it (2026-11-05), from the ways people write one: 2026/11/05,
  // 05.11.2026 (day first), 11/05/2026 (month first, unless the first number cannot be a month),
  // "Nov 5, 2026", "5 November 2026". Anything else is left as it is.
  const MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec']
  function isoDate(text) {
    const t = String(text).trim()
    const pad = (n) => String(n).padStart(2, '0')
    if (/^\d{4}-\d{2}-\d{2}$/.test(t)) return t
    let m = t.match(/^(\d{4})[./-](\d{1,2})[./-](\d{1,2})$/)
    if (m) return `${m[1]}-${pad(m[2])}-${pad(m[3])}`
    m = t.match(/^(\d{1,2})([./-])(\d{1,2})\2(\d{4})$/)
    if (m) {
      const [a, b] = [Number(m[1]), Number(m[3])]
      const [day, month] = m[2] === '.' || a > 12 ? [a, b] : [b, a]
      return `${m[4]}-${pad(month)}-${pad(day)}`
    }
    const month = MONTHS.findIndex((name) => new RegExp(`\\b${name}`, 'i').test(t))
    const day = t.match(/\b(\d{1,2})(?:st|nd|rd|th)?\b/)
    const year = t.match(/\b(\d{4})\b/)
    if (month >= 0 && day && year) return `${year[1]}-${pad(month + 1)}-${pad(day[1])}`
    return t
  }
