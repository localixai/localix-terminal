  // The page's own requests, for whether it is still working (`busy`) and for how long an action
  // waits for the answers it asked for (`settle`). Fetch and XHR are wrapped as the probe starts,
  // before any page script. Only the page's own site counts, by its registrable domain (the
  // calls www.shop.com makes to api.shop.com are its own); third-party traffic (ads, analytics)
  // never ends and is not the page's content. A request repeated at a steady pace (a poll, a
  // heartbeat) is the page living, not working: from its third start at a steady interval its
  // shape no longer counts. A request older than INFLIGHT_MAX is a long poll.
  const INFLIGHT_MAX = 5000
  // A request this soon after an action began is the action's, never taken for a poll.
  const CAUSED_MS = 1000
  const inflight = new Map()
  const starts = new Map()
  const polls = new Set()
  let requests = 0
  // When the latest action began (`begin`): its requests are the ones `settle` waits for.
  let opStart = -Infinity

  // An action starts: a document it replaces is noticed from here, and what the page announces
  // or asks from here on is the action's (told.js).
  function begin() {
    leaving = false
    opStart = performance.now()
    listen()
    quietFrames()
  }

  // The registrable domain of `host`, by a rule of thumb that needs no list: the last two labels,
  // or three under a short second level of a country's domain (shop.co.uk).
  const SHORT_SECOND = /^(co|com|net|org|gov|edu|ac|ne|or|go|gob|nic|mil)$/
  const siteOf = (host) => {
    if (!host.includes('.') || /^[\d.]+$/.test(host) || host.includes(':')) return host
    const parts = host.split('.')
    const country = parts.length > 2 && parts[parts.length - 1].length === 2
    return parts.slice(country && SHORT_SECOND.test(parts[parts.length - 2]) ? -3 : -2).join('.')
  }
  const own = (url) => {
    try { return siteOf(new URL(url, location.href).hostname) === siteOf(location.hostname) } catch { return false }
  }

  // Whether `shape` (a method and an address, digits blurred) is polled: three starts or more,
  // the last two intervals at least 50 ms and within half of each other.
  const polled = (shape, now) => {
    if (polls.has(shape)) return true
    const times = (starts.get(shape) || []).concat(now).slice(-4)
    starts.delete(shape)
    starts.set(shape, times)
    if (starts.size > 200) starts.delete(starts.keys().next().value)
    if (times.length < 3) return false
    const a = times[times.length - 2] - times[times.length - 3]
    const b = times[times.length - 1] - times[times.length - 2]
    if (a < 50 || b < 50 || Math.abs(a - b) > Math.max(a, b) / 2) return false
    polls.add(shape)
    return true
  }

  const track = (method, url, promise) => {
    if (!own(url)) return
    const now = performance.now()
    const shape = `${String(method || 'GET').toUpperCase()} ${String(url).replace(/\d+/g, '#')}`
    if (polled(shape, now) && now - opStart > CAUSED_MS) return
    const id = ++requests
    inflight.set(id, now)
    const done = () => inflight.delete(id)
    promise.then(done, done)
  }
  const nativeFetch = window.fetch
  if (nativeFetch) {
    window.fetch = function (input, init) {
      const result = nativeFetch.call(this, input, init)
      track(init?.method || input?.method, input?.url ?? String(input), result)
      return result
    }
  }
  const nativeOpen = XMLHttpRequest.prototype.open
  const nativeSend = XMLHttpRequest.prototype.send
  const requested = new WeakMap()
  XMLHttpRequest.prototype.open = function (method, url, ...rest) {
    requested.set(this, [method, String(url)])
    return nativeOpen.call(this, method, url, ...rest)
  }
  XMLHttpRequest.prototype.send = function (body) {
    const [method, url] = requested.get(this) || ['GET', '']
    track(method, url, new Promise((resolve) => this.addEventListener('loadend', resolve, { once: true })))
    return nativeSend.call(this, body)
  }

  const fresh = (start) => performance.now() - start < INFLIGHT_MAX
  const fetching = () => [...inflight.values()].some(fresh)
  // Whether a request the latest action started is still out.
  const awaited = () => [...inflight.values()].some((start) => start >= opStart && fresh(start))
