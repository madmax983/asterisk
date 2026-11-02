// Asterisk — the one script. Wired by data- attributes; no inline handlers.
(function () {
  'use strict';
  var token = (document.querySelector('meta[name="csrf-token"]') || {}).content || '';
  var page = document.body.getAttribute('data-page');

  // Your colour: one per browser, so your rules are yours on every tree.
  var hue = NaN;
  try { hue = parseInt(localStorage.getItem('asterisk-hue'), 10); } catch (e) { hue = NaN; }
  if (!(hue >= 0 && hue < 360)) {
    hue = Math.floor(Math.random() * 360);
    try { localStorage.setItem('asterisk-hue', String(hue)); } catch (e) { /* private window */ }
  }

  function post(url, body) {
    return fetch(url, {
      method: 'POST', credentials: 'same-origin',
      headers: { 'x-csrf-token': token, 'content-type': 'application/x-www-form-urlencoded' },
      body: body || ''
    });
  }
  function enc(o) {
    return Object.keys(o).map(function (k) { return encodeURIComponent(k) + '=' + encodeURIComponent(o[k]); }).join('&');
  }
  function swap(id, url) {
    return fetch(url, { credentials: 'same-origin' }).then(function (r) { return r.ok ? r.text() : null; })
      .then(function (html) { var el = document.getElementById(id); if (html !== null && el) { el.innerHTML = html; } });
  }
  function listen(fn) {
    if (!window.EventSource) { return; }
    var seq = 0, es = new EventSource('/stream');
    es.onmessage = function (ev) {
      var b; try { b = JSON.parse(ev.data); } catch (e) { return; }
      if (b.seq <= seq) { return; }
      seq = b.seq; fn(b);
    };
  }

  window.asterisk = { hue: hue };

  if (page === 'gallery') {
    var pending = null;
    listen(function () {
      if (pending) { return; }
      pending = setTimeout(function () {
        pending = null;
        fetch('/', { credentials: 'same-origin' }).then(function (r) { return r.text(); }).then(function (html) {
          var doc = new DOMParser().parseFromString(html, 'text/html');
          var fresh = doc.querySelector('.cards'), here = document.querySelector('.cards');
          if (fresh && here) { here.innerHTML = fresh.innerHTML; }
        });
      }, 2500);
    });
    return;
  }

  if (page === 'new') {
    var form = document.getElementById('newform'), vocab = document.getElementById('vocab');
    document.getElementById('use-example').addEventListener('click', function () { vocab.value = vocab.getAttribute('placeholder'); });
    document.getElementById('use-standard').addEventListener('click', function (ev) { vocab.value = ev.target.getAttribute('data-text'); });
    form.addEventListener('submit', function (ev) {
      ev.preventDefault();
      var out = document.getElementById('newresult');
      out.textContent = 'growing…';
      post('/families', enc({
        title: form.title.value, seed: form.seed.value, span: form.span.value,
        century_secs: form.century_secs.value, vocab: vocab.value
      })).then(function (r) {
        return r.ok ? r.json().then(function (c) { location.href = c.href; })
          : r.text().then(function (t) {
            var msg = t; try { var j = JSON.parse(t); msg = j.detail || j.message || j.error || t; } catch (e) { /* plain text */ }
            out.textContent = msg; out.className = 'preview error';
          });
      });
    });
    return;
  }

  if (page !== 'family') { return; }

  var clock = document.querySelector('.clock');
  var base = clock.getAttribute('data-base'), slug = clock.getAttribute('data-family');
  var stage = document.querySelector('.stage');
  var m = stage.getAttribute('data-m');
  var me = document.getElementById('me');
  if (me) { me.style.background = 'hsl(' + hue + ' 85% 62%)'; me.style.color = 'hsl(' + hue + ' 85% 62%)'; }

  function show(gloss) {
    m = gloss;
    stage.setAttribute('data-m', m);
    document.querySelectorAll('#meanings .chip').forEach(function (c) { c.classList.toggle('on', c.getAttribute('data-m') === m); });
    history.replaceState(null, '', base + '?m=' + encodeURIComponent(m));
    swap('tree', base + '/_tree?m=' + encodeURIComponent(m));
  }
  document.getElementById('meanings').addEventListener('click', function (ev) {
    var a = ev.target.closest('a[data-m]');
    if (!a) { return; }
    ev.preventDefault(); show(a.getAttribute('data-m'));
  });
  var find = document.getElementById('find'), known = {};
  document.querySelectorAll('#glosses option').forEach(function (o) { known[o.value] = true; });
  function tryFind() { var g = find.value.trim(); if (known[g]) { show(g); } }
  find.addEventListener('change', tryFind);
  find.addEventListener('keydown', function (ev) { if (ev.key === 'Enter') { ev.preventDefault(); tryFind(); } });

  var drift = document.getElementById('drift');
  function showDrift(on) {
    if (!drift) { return; }
    drift.setAttribute('data-on', on ? 'true' : 'false');
    drift.classList.toggle('on', on);
    drift.textContent = on ? 'time is running' : 'time is stopped';
  }
  if (drift) {
    drift.addEventListener('click', function () {
      var on = drift.getAttribute('data-on') !== 'true';
      post(base + '/drift/' + (on ? 'on' : 'off')).then(function (r) { return r.json(); }).then(function (s) { showDrift(s.drifting); });
    });
  }
  var once = document.getElementById('once');
  if (once) { once.addEventListener('click', function () { post(base + '/century'); }); }

  var rule = document.getElementById('rule');
  var result = document.getElementById('result');
  function target() { var t = document.getElementById('target'); return t ? t.value : 'all'; }
  function send(url) {
    var src = rule.value.trim();
    if (!src) { rule.focus(); return; }
    post(url, enc({ rule: src, target: target(), hue: hue }))
      .then(function (r) { return r.text(); })
      .then(function (html) { result.innerHTML = html; });
  }
  document.getElementById('preview').addEventListener('click', function () { send(base + '/preview'); });
  document.getElementById('apply').addEventListener('click', function () { send(base + '/apply'); });
  rule.addEventListener('keydown', function (ev) {
    if (ev.key === 'Enter' && !ev.shiftKey) { ev.preventDefault(); send(base + (ev.metaKey || ev.ctrlKey ? '/apply' : '/preview')); }
  });
  document.querySelector('.palette').addEventListener('click', function (ev) {
    var b = ev.target.closest('button[data-glyph]');
    if (!b) { return; }
    var g = b.getAttribute('data-glyph');
    var s = rule.selectionStart, e = rule.selectionEnd, v = rule.value;
    rule.value = v.slice(0, s) + g + v.slice(e);
    rule.focus();
    rule.selectionStart = rule.selectionEnd = s + g.length;
  });
  document.querySelector('.try').addEventListener('click', function (ev) {
    var b = ev.target.closest('button[data-rule]');
    if (!b) { return; }
    rule.value = b.getAttribute('data-rule');
    send(base + '/preview');
  });

  var century = document.getElementById('century');
  var spanEl = document.getElementById('span');
  listen(function (b) {
    if (b.family !== slug) { return; }
    showDrift(b.drifting);
    if (b.kind === 'complete') { location.reload(); return; }
    if (String(b.century) !== century.textContent) {
      century.textContent = b.century;
      if (spanEl) { spanEl.textContent = spanEl.textContent.replace(/^\d+/, String(b.century)); }
      var c = century.parentNode;
      c.classList.remove('tick'); void c.offsetWidth; c.classList.add('tick');
    }
    if (b.kind === 'drift') { return; }
    swap('tree', base + '/_tree?m=' + encodeURIComponent(m));
    swap('feed', base + '/_feed');
    var keep = target();
    swap('targets', base + '/_targets').then(function () {
      var t = document.getElementById('target');
      if (t && Array.prototype.some.call(t.options, function (o) { return o.value === keep; })) { t.value = keep; }
    });
  });
})();
