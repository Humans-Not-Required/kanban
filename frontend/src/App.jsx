import { useState, useEffect, useCallback, useRef } from 'react';
import * as api from './api';

// ---- UTC timestamp parsing ----
// API returns timestamps like "2026-02-09 16:42:27" (UTC, no timezone marker).
// Ensure they're always parsed as UTC so the browser displays in local timezone.
function parseUTC(ts) {
  if (!ts) return new Date(NaN);
  let s = String(ts).trim();
  // Replace space separator with 'T' for ISO 8601 compliance
  if (/^\d{4}-\d{2}-\d{2} /.test(s)) s = s.replace(' ', 'T');
  // Append 'Z' if no timezone info present
  if (!s.includes('Z') && !s.includes('+') && !/T\d{2}:\d{2}(:\d{2})?[-+]/.test(s)) s += 'Z';
  return new Date(s);
}

// ---- Label normalization ----
// Lowercase, trim, collapse multiple spaces, replace spaces with dashes
function normalizeLabel(label) {
  return label.toLowerCase().trim().replace(/\s+/g, '-').replace(/-+/g, '-').replace(/^-|-$/g, '');
}
function normalizeLabels(labelsStr) {
  if (!labelsStr || !labelsStr.trim()) return [];
  return labelsStr.split(',').map(l => normalizeLabel(l)).filter(Boolean);
}

// ---- iOS Safari zoom reset on app resume ----
// When Safari auto-zooms on input focus and the user switches apps,
// the zoom persists when returning. This resets it.
if (typeof document !== 'undefined') {
  // Detect iOS
  const isIOS = /iPad|iPhone|iPod/.test(navigator.userAgent) ||
    (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);

  if (isIOS) {
    // On iOS Safari, maximum-scale=1 prevents auto-zoom on input focus
    // but does NOT block user-initiated pinch-to-zoom (iOS 10+)
    const viewport = document.querySelector('meta[name="viewport"]');
    if (viewport) {
      viewport.content = 'width=device-width, initial-scale=1.0, maximum-scale=1.0';
    }
  }

  // Reset zoom when returning from another app (belt and suspenders)
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
      const viewport = document.querySelector('meta[name="viewport"]');
      if (viewport) {
        // Temporarily force scale reset, then restore
        const original = viewport.content;
        viewport.content = 'width=device-width, initial-scale=1.0, maximum-scale=1.0';
        setTimeout(() => { viewport.content = original; }, 100);
      }
    }
  });
}

// ---- Escape key hook (layered: only topmost modal closes) ----
let escapeLayerId = 0;
const escapeStack = [];
function useEscapeKey(onClose) {
  useEffect(() => {
    const id = ++escapeLayerId;
    escapeStack.push(id);
    const handler = (e) => {
      if (e.key === 'Escape' && escapeStack[escapeStack.length - 1] === id) {
        e.stopImmediatePropagation();
        onClose();
      }
    };
    document.addEventListener('keydown', handler);
    return () => {
      document.removeEventListener('keydown', handler);
      const idx = escapeStack.indexOf(id);
      if (idx !== -1) escapeStack.splice(idx, 1);
    };
  }, [onClose]);
}

// ---- Autocomplete input ----
function AutocompleteInput({ value, onChange, suggestions, placeholder, style, isCommaList }) {
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [focusedIdx, setFocusedIdx] = useState(-1);

  // For comma-separated lists, get the current token being typed
  const getCurrentToken = () => {
    if (!isCommaList) return value;
    const parts = value.split(',');
    return (parts[parts.length - 1] || '').trim();
  };

  const getExistingTokens = () => {
    if (!isCommaList) return [];
    return value.split(',').slice(0, -1).map(t => t.trim().toLowerCase()).filter(Boolean);
  };

  const currentToken = getCurrentToken().toLowerCase();
  const existing = getExistingTokens();
  const filtered = suggestions.filter(s =>
    s.toLowerCase().includes(currentToken) &&
    !existing.includes(s.toLowerCase()) &&
    s.toLowerCase() !== currentToken
  );

  const selectSuggestion = (suggestion) => {
    if (isCommaList) {
      const parts = value.split(',').slice(0, -1).map(t => t.trim()).filter(Boolean);
      parts.push(suggestion);
      onChange(parts.join(', ') + ', ');
    } else {
      onChange(suggestion);
    }
    setShowSuggestions(false);
    setFocusedIdx(-1);
  };

  const handleKeyDown = (e) => {
    if (!showSuggestions || filtered.length === 0) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setFocusedIdx(i => Math.min(i + 1, filtered.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setFocusedIdx(i => Math.max(i - 1, -1));
    } else if (e.key === 'Tab' || e.key === 'Enter') {
      if (focusedIdx >= 0 && focusedIdx < filtered.length) {
        e.preventDefault();
        selectSuggestion(filtered[focusedIdx]);
      }
    }
  };

  return (
    <div style={{ position: 'relative' }}>
      <input
        style={style}
        placeholder={placeholder}
        value={value}
        onChange={e => { onChange(e.target.value); setShowSuggestions(true); setFocusedIdx(-1); }}
        onFocus={() => setShowSuggestions(true)}
        onBlur={() => setTimeout(() => setShowSuggestions(false), 150)}
        onKeyDown={handleKeyDown}
      />
      {showSuggestions && currentToken.length > 0 && filtered.length > 0 && (
        <div style={{
          position: 'absolute', top: '100%', left: 0, right: 0, zIndex: 1000,
          background: '#1e293b', border: '1px solid #475569', borderRadius: '6px',
          maxHeight: '150px', overflowY: 'auto', marginTop: '2px',
        }}>
          {filtered.slice(0, 8).map((s, i) => (
            <div
              key={s}
              onMouseDown={() => selectSuggestion(s)}
              style={{
                padding: '6px 10px', cursor: 'pointer', fontSize: '13px',
                color: i === focusedIdx ? '#f1f5f9' : '#94a3b8',
                background: i === focusedIdx ? '#334155' : 'transparent',
              }}
            >{s}</div>
          ))}
        </div>
      )}
    </div>
  );
}

// ---- Styled Select (custom chevron, consistent across platforms) ----
function StyledSelect({ style, children, ...props }) {
  const wrapperStyle = {
    position: 'relative',
    display: 'inline-flex',
    flex: style?.flex ?? 1,
    minWidth: style?.minWidth ?? undefined,
    marginBottom: style?.marginBottom ?? undefined,
    gridColumn: style?.gridColumn ?? undefined,
    width: style?.width ?? undefined,
  };
  const chevronStyle = {
    position: 'absolute',
    right: '10px',
    top: '50%',
    transform: 'translateY(-50%)',
    pointerEvents: 'none',
    display: 'flex',
    alignItems: 'center',
  };
  // Merge caller styles, force appearance:none and right padding for chevron
  const selectStyle = {
    ...style,
    flex: 1,
    width: '100%',
    appearance: 'none',
    WebkitAppearance: 'none',
    MozAppearance: 'none',
    paddingRight: '32px',
    cursor: 'pointer',
    // remove wrapper-level props that don't belong on <select>
    minWidth: undefined,
    gridColumn: undefined,
  };
  return (
    <div style={wrapperStyle}>
      <select style={selectStyle} {...props}>
        {children}
      </select>
      <span style={chevronStyle}>
        <svg width="12" height="8" viewBox="0 0 12 8" fill="none" xmlns="http://www.w3.org/2000/svg">
          <path d="M1.5 1.75L6 6.25L10.5 1.75" stroke="#94a3b8" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
        </svg>
      </span>
    </div>
  );
}

// ---- Responsive hook ----
function useBreakpoint() {
  const [width, setWidth] = useState(window.innerWidth);
  useEffect(() => {
    const handler = () => setWidth(window.innerWidth);
    window.addEventListener('resize', handler);
    return () => window.removeEventListener('resize', handler);
  }, []);
  return { isMobile: width < 768, isCompact: width < 1024 };
}

// ---- Styles ----
const styles = {
  app: (mobile) => (mobile
    ? { minHeight: '100dvh', display: 'block', overflowX: 'hidden' }
    : { height: '100dvh', display: 'flex', flexDirection: 'column', overflow: 'hidden' }
  ),
  header: (mobile) => ({
    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
    padding: mobile ? '8px 10px' : '12px 20px', background: '#1e293b',
    borderBottom: '1px solid #334155',
    minHeight: mobile ? '40px' : '48px', overflow: 'visible',
    gap: '8px',
  }),
  logo: { fontSize: '1.2rem', fontWeight: 700, color: '#f1f5f9', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '8px', flexShrink: 0 },
  logoImg: { width: '24px', height: '24px' },
  headerRight: { display: 'flex', alignItems: 'center', gap: '6px', fontSize: '0.85rem', flexShrink: 1, minWidth: 0 },
  menuBtn: {
    background: '#1e293b', border: '1px solid #334155', color: '#94a3b8',
    padding: '7px', borderRadius: '6px', cursor: 'pointer',
    lineHeight: 0, transition: 'background 0.15s, border-color 0.15s, color 0.15s',
    display: 'flex', alignItems: 'center', justifyContent: 'center',
    width: '34px', height: '34px',
  },
  modeBadge: (canEdit) => ({
    fontSize: '0.7rem', fontWeight: 600,
    padding: '3px 8px', borderRadius: '12px',
    background: canEdit ? '#22c55e22' : '#64748b22',
    color: canEdit ? '#22c55e' : '#94a3b8',
    border: `1px solid ${canEdit ? '#22c55e44' : '#64748b44'}`,
    whiteSpace: 'nowrap',
  }),
  main: (mobile) => ({
    flex: mobile ? undefined : 1,
    display: 'flex',
    flexDirection: mobile ? 'column' : 'row',
    overflow: mobile ? 'visible' : 'hidden',
    position: 'relative',
  }),
  sidebar: (mobile, open) => ({
    ...(mobile ? {
      position: 'fixed', top: 0, left: 0, bottom: 0,
      width: '280px', maxWidth: '85vw', zIndex: 200,
      transform: open ? 'translateX(0)' : 'translateX(-100%)',
      transition: 'transform 0.2s ease',
    } : {
      width: '240px', minWidth: '240px',
    }),
    background: '#1e293b',
    borderRight: '1px solid #334155', display: 'flex', flexDirection: 'column',
    overflow: 'auto',
  }),
  sidebarOverlay: {
    position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.5)', zIndex: 199,
  },
  sidebarHeader: {
    padding: '12px 16px', fontSize: '0.75rem', fontWeight: 600, color: '#94a3b8',
    textTransform: 'uppercase', letterSpacing: '0.05em',
    display: 'flex', justifyContent: 'space-between', alignItems: 'center',
  },
  boardItem: (active) => ({
    padding: '10px 16px', cursor: 'pointer', fontSize: '0.9rem',
    background: active ? '#334155' : 'transparent',
    color: active ? '#f1f5f9' : '#94a3b8',
    borderLeft: active ? '3px solid #6366f1' : '3px solid transparent',
    display: 'flex', justifyContent: 'space-between', alignItems: 'center',
  }),
  archivedBadge: {
    fontSize: '0.65rem', background: '#475569', color: '#94a3b8',
    padding: '1px 5px', borderRadius: '3px',
  },
  boardContent: (mobile) => ({
    flex: mobile ? undefined : 1,
    display: 'flex',
    flexDirection: 'column',
    overflow: mobile ? 'visible' : 'hidden',
    position: 'relative',
  }),
  boardHeader: (mobile) => ({
    padding: mobile ? '12px' : '16px 20px',
    display: 'flex', alignItems: mobile ? 'flex-start' : 'center',
    justifyContent: 'space-between',
    flexDirection: mobile ? 'column' : 'row', gap: mobile ? '8px' : '0',
  }),
  boardTitle: (mobile) => ({
    fontSize: mobile ? '1.1rem' : '1.3rem', fontWeight: 700, color: '#f1f5f9',
  }),
  columnsContainer: (mobile) => ({
    flex: mobile ? undefined : 1,
    display: 'flex',
    flexDirection: mobile ? 'column' : 'row',
    gap: mobile ? '12px' : '16px',
    padding: mobile ? '12px' : '16px 20px',
    overflowX: mobile ? 'hidden' : 'auto',
    overflowY: mobile ? 'visible' : 'hidden',
    alignItems: mobile ? 'stretch' : 'stretch',
    minHeight: 0,
  }),
  column: (isDragOver, mobile) => ({
    ...(mobile ? {
      flex: 'none', width: '100%',
    } : {
      minWidth: '280px', maxWidth: '320px', flex: '0 0 280px',
    }),
    background: isDragOver ? '#1e293b' : '#1a2332', borderRadius: '8px',
    border: isDragOver ? '2px dashed #6366f1' : '1px solid #334155',
    display: 'flex', flexDirection: 'column',
    maxHeight: mobile ? 'none' : '100%',
  }),
  columnHeader: {
    padding: '12px 14px', fontWeight: 600, fontSize: '0.9rem',
    display: 'flex', justifyContent: 'space-between', alignItems: 'center',
    borderBottom: '1px solid #334155', color: '#e2e8f0',
  },
  taskCount: {
    fontSize: '0.75rem', color: '#64748b', background: '#0f172a',
    padding: '2px 8px', borderRadius: '10px',
  },
  taskList: (mobile) => ({
    flex: mobile ? 'none' : 1,
    overflow: mobile ? 'visible' : 'auto',
    padding: '8px',
  }),
  card: (isDragging, priority) => ({
    background: isDragging ? '#334155' : '#0f172a',
    border: `1px solid ${priorityColor(priority)}33`,
    borderLeft: `3px solid ${priorityColor(priority)}`,
    borderRadius: '6px', padding: '10px 12px', marginBottom: '8px',
    cursor: isDragging ? 'grabbing' : 'pointer',
    opacity: isDragging ? 0.5 : 1,
    transition: 'all 0.15s ease',
  }),
  cardDraggable: { cursor: 'grab' },
  cardTitle: { fontSize: '0.88rem', fontWeight: 500, color: '#e2e8f0', marginBottom: '4px' },
  cardMeta: { display: 'flex', gap: '8px', fontSize: '0.73rem', color: '#64748b', flexWrap: 'wrap' },
  label: (color) => ({
    background: color || '#6366f133', color: color ? '#fff' : '#a5b4fc',
    padding: '1px 6px', borderRadius: '3px', fontSize: '0.68rem',
  }),
  btn: (variant = 'primary', mobile) => ({
    background: variant === 'primary' ? '#6366f1' : variant === 'danger' ? '#ef4444' : '#334155',
    color: '#fff', border: 'none',
    padding: mobile ? '8px 14px' : '6px 12px',
    borderRadius: '4px', cursor: 'pointer',
    fontSize: mobile ? '0.85rem' : '0.8rem', fontWeight: 500,
    whiteSpace: 'nowrap',
    height: '32px', lineHeight: '1', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
    boxSizing: 'border-box',
  }),
  btnSmall: {
    background: '#334155', border: '1px solid #475569', color: '#cbd5e1',
    padding: '3px 8px', borderRadius: '4px', cursor: 'pointer', fontSize: '0.75rem',
    height: '32px', lineHeight: '1', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
    boxSizing: 'border-box', outline: 'none',
  },
  btnClose: {
    background: 'transparent', border: '1px solid #334155', color: '#94a3b8',
    width: '32px', height: '32px', borderRadius: '4px', cursor: 'pointer',
    fontSize: '1rem', lineHeight: 1, padding: 0,
    display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
    boxSizing: 'border-box', flexShrink: 0,
  },
  btnIcon: {
    background: '#334155', border: '1px solid #475569', color: '#cbd5e1',
    width: '32px', height: '32px', borderRadius: '4px', cursor: 'pointer',
    fontSize: '0.8rem', lineHeight: 1, padding: 0,
    display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
    boxSizing: 'border-box', flexShrink: 0,
  },
  modal: (mobile) => ({
    position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.6)',
    display: 'flex', alignItems: mobile ? 'stretch' : 'flex-start', justifyContent: 'center', zIndex: 1100,
    padding: mobile ? '0' : '12px',
    paddingTop: mobile ? '0' : '4vh',
  }),
  modalContent: (mobile) => ({
    background: '#1e293b', border: mobile ? 'none' : '1px solid #334155', borderRadius: mobile ? '0' : '8px',
    padding: mobile ? '16px' : '24px', paddingBottom: mobile ? '24px' : '24px',
    width: mobile ? '100%' : '480px', maxWidth: '100%',
    maxHeight: mobile ? '100dvh' : '90vh', height: mobile ? '100dvh' : 'auto', overflow: 'auto',
  }),
  modalContentWide: (mobile) => ({
    background: '#1e293b', border: mobile ? 'none' : '1px solid #334155', borderRadius: mobile ? '0' : '8px',
    padding: mobile ? '16px' : '24px', paddingBottom: mobile ? '24px' : '24px',
    width: mobile ? '100%' : '680px', maxWidth: '100%',
    maxHeight: mobile ? '100dvh' : '90vh', height: mobile ? '100dvh' : 'auto', overflow: 'auto',
  }),
  input: {
    width: '100%', background: '#0f172a', border: '1px solid #334155', color: '#e2e8f0',
    padding: '10px', borderRadius: '4px', fontSize: '16px', marginBottom: '10px',
    boxSizing: 'border-box',
  },
  textarea: {
    width: '100%', background: '#0f172a', border: '1px solid #334155', color: '#e2e8f0',
    padding: '10px', borderRadius: '4px', fontSize: '16px', minHeight: '140px',
    resize: 'vertical', marginBottom: '10px', fontFamily: 'inherit',
    boxSizing: 'border-box',
  },
  select: {
    background: '#0f172a', border: '1px solid #334155', color: '#e2e8f0',
    padding: '8px', borderRadius: '4px', fontSize: '16px', marginBottom: '10px',
    flex: 1, boxSizing: 'border-box',
  },
  empty: {
    textAlign: 'center', color: '#475569', padding: '40px 20px', fontSize: '0.9rem',
  },
  searchBar: (mobile) => ({
    display: 'flex', gap: '8px',
    padding: mobile ? '0 12px' : '0 20px',
    paddingBottom: '0',
  }),
  urlBox: {
    background: '#0f172a', border: '1px solid #334155', borderRadius: '4px',
    padding: '10px 12px', fontSize: '0.78rem', color: '#94a3b8',
    fontFamily: 'monospace', wordBreak: 'break-all', marginBottom: '10px',
    display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '8px',
  },
  urlLabel: {
    fontSize: '0.73rem', fontWeight: 600, color: '#64748b',
    textTransform: 'uppercase', marginBottom: '4px',
  },
  successBox: {
    background: '#22c55e11', border: '1px solid #22c55e33', borderRadius: '8px',
    padding: '16px', marginBottom: '16px',
  },
  directBoardInput: {
    background: '#0f172a', border: '1px solid #334155', color: '#e2e8f0',
    padding: '8px 10px', borderRadius: '4px', fontSize: '16px', flex: 1,
    minWidth: 0, boxSizing: 'border-box',
  },
};

function priorityColor(p) {
  // Handle both string and integer priorities
  if (p === 'critical' || p >= 3) return '#ef4444';
  if (p === 'high' || p === 2) return '#f97316';
  if (p === 'medium' || p === 1) return '#eab308';
  if (p === 'low' || p === 0) return '#22c55e';
  return '#64748b';
}

function priorityLabel(p) {
  if (p === 'critical' || p >= 3) return 'critical';
  if (p === 'high' || p === 2) return 'high';
  if (p === 'medium' || p === 1) return 'medium';
  if (p === 'low' || p === 0) return 'low';
  return String(p);
}

// ---- Priority Toggle (4-way button bar) ----

const PRIORITY_OPTIONS = [
  { value: 3, label: 'Critical', color: '#ef4444' },
  { value: 2, label: 'High', color: '#f97316' },
  { value: 1, label: 'Medium', color: '#eab308' },
  { value: 0, label: 'Low', color: '#22c55e' },
];

function PriorityToggle({ value, onChange, compact = false }) {
  return (
    <div style={{
      display: 'flex',
      borderRadius: '6px',
      overflow: 'hidden',
      border: '1px solid #475569',
      flex: 1,
      minHeight: '32px',
      boxSizing: 'border-box',
    }}>
      {PRIORITY_OPTIONS.map((opt, i) => {
        const isActive = value === opt.value;
        return (
          <button
            key={opt.value}
            type="button"
            onClick={() => onChange(opt.value)}
            aria-label={opt.label}
            title={opt.label}
            style={{
              flex: 1,
              padding: compact ? 0 : '6px 0',
              fontSize: compact ? '0.75rem' : '0.78rem',
              fontWeight: isActive ? '700' : '500',
              color: isActive ? '#fff' : '#94a3b8',
              background: isActive ? opt.color + 'cc' : '#1e293b',
              border: 'none',
              borderRight: i < PRIORITY_OPTIONS.length - 1 ? '1px solid #475569' : 'none',
              cursor: 'pointer',
              transition: 'background 0.15s, color 0.15s',
              lineHeight: '1.2',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              userSelect: 'none',
            }}
          >
            {compact ? (
              <span
                style={{
                  width: '12px',
                  height: '12px',
                  borderRadius: '999px',
                  background: opt.color,
                  boxShadow: isActive ? '0 0 0 2px rgba(255,255,255,0.35)' : '0 0 0 1px rgba(0,0,0,0.35)',
                }}
              />
            ) : (
              opt.label
            )}
          </button>
        );
      })}
    </div>
  );
}

// ---- Copy to clipboard helper ----

function copyToClipboard(text) {
  navigator.clipboard.writeText(text).then(
    () => {},
    () => {
      const ta = document.createElement('textarea');
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
    }
  );
}

// ---- @Mention Rendering ----

/** Render text with @mentions highlighted. Returns array of React elements. */
function renderWithMentions(text) {
  if (!text) return text;
  // Match @"Quoted Name" or @word-chars
  const mentionRegex = /@"([^"]+)"|@([\w._-]+)/g;
  const parts = [];
  let lastIndex = 0;
  let match;
  let key = 0;
  while ((match = mentionRegex.exec(text)) !== null) {
    if (match.index > lastIndex) {
      parts.push(text.slice(lastIndex, match.index));
    }
    const name = match[1] || match[2];
    const displayName = api.getDisplayName();
    const isMe = displayName && name.toLowerCase() === displayName.toLowerCase();
    parts.push(
      <span key={key++} style={{
        color: isMe ? '#fbbf24' : '#818cf8',
        fontWeight: 600,
        cursor: 'default',
      }} title={`@${name}`}>@{name}</span>
    );
    lastIndex = match.index + match[0].length;
  }
  if (lastIndex < text.length) {
    parts.push(text.slice(lastIndex));
  }
  return parts.length > 0 ? parts : text;
}

// ---- Components ----

function IdentityBadge({ isMobile }) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(() => api.getDisplayName());
  const [inputVal, setInputVal] = useState(name);

  const save = () => {
    const trimmed = inputVal.trim();
    api.setDisplayName(trimmed);
    setName(trimmed);
    setEditing(false);
  };

  if (editing) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
        <input
          style={{
            background: '#0f172a', border: '1px solid #6366f1', color: '#e2e8f0',
            padding: '3px 8px', borderRadius: '4px', fontSize: '16px',
            width: isMobile ? '100px' : '120px',
          }}
          placeholder="Your name"
          value={inputVal}
          onChange={e => setInputVal(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter') save(); if (e.key === 'Escape') setEditing(false); }}
          autoFocus
        />
        <button
          style={{ ...styles.btnSmall, padding: '3px 6px', fontSize: '0.75rem' }}
          onClick={save}
        >✓</button>
      </div>
    );
  }

  return (
    <span
      style={{
        fontSize: '0.78rem', color: name ? '#a5b4fc' : '#64748b',
        cursor: 'pointer', padding: '3px 8px',
        background: '#0f172a33', borderRadius: '4px',
        border: '1px solid #334155',
        whiteSpace: 'nowrap', maxWidth: isMobile ? '90px' : '140px',
        overflow: 'hidden', textOverflow: 'ellipsis', display: 'inline-block',
      }}
      onClick={() => { setInputVal(name); setEditing(true); }}
      title={name ? `Signed in as "${name}" — click to change` : 'Set your display name'}
    >
      {name ? `👤 ${name}` : '👤 Set name'}
    </span>
  );
}

function TaskCard({ task, boardId, canEdit, onRefresh, archived, onClickTask, isMobile }) {
  const [dragging, setDragging] = useState(false);
  const draggable = canEdit && !archived && !isMobile;

  return (
    <div
      style={{
        ...styles.card(dragging, task.priority),
        ...(draggable ? styles.cardDraggable : {}),
        cursor: dragging ? 'grabbing' : 'pointer',
      }}
      draggable={draggable}
      onDragStart={draggable ? (e) => { setDragging(true); e.dataTransfer.setData('taskId', task.id); } : undefined}
      onDragEnd={draggable ? () => setDragging(false) : undefined}
      onClick={(e) => { e.stopPropagation(); if (!dragging) onClickTask(task); }}
    >
      <div style={styles.cardTitle}>{task.title || (task.description ? task.description.slice(0, 60) + (task.description.length > 60 ? '…' : '') : '(untitled)')}</div>
      <div style={styles.cardMeta}>
        <span style={{ color: priorityColor(task.priority) }}>{priorityLabel(task.priority)}</span>
        {task.assigned_to && <span>→ {task.assigned_to}</span>}
        {task.claimed_by && <span>🔒 {task.claimed_by}</span>}
        {task.due_at && <span>📅 {parseUTC(task.due_at).toLocaleDateString()}</span>}
        {task.completed_at && <span>✅</span>}
        {task.archived_at && <span>📦</span>}
        {task.comment_count > 0 && <span>💬 {task.comment_count}</span>}
      </div>
      {task.labels && task.labels.length > 0 && (
        <div style={{ display: 'flex', gap: '4px', marginTop: '6px', flexWrap: 'wrap' }}>
          {task.labels.map((l, i) => <span key={i} style={styles.label()}>{l}</span>)}
        </div>
      )}
    </div>
  );
}

function MoveTaskDropdown({ boardId, task, columns, onMoved, onCancel }) {
  const otherColumns = columns.filter(c => c.id !== task.column_id);
  const handleMove = async (columnId) => {
    try {
      await api.moveTask(boardId, task.id, columnId);
      onMoved();
    } catch (err) {
      if (err.code === 'WIP_LIMIT_EXCEEDED') {
        alert(`WIP limit reached for that column`);
      }
    }
  };
  return (
    <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap', marginTop: '8px' }}>
      {otherColumns.map(c => (
        <button key={c.id} style={{ ...styles.btnSmall, padding: '6px 10px' }} onClick={() => handleMove(c.id)}>
          → {c.name}
        </button>
      ))}
      <button style={{ ...styles.btnSmall, padding: '6px 10px', color: '#ef4444' }} onClick={onCancel}>Cancel</button>
    </div>
  );
}

const TASKS_PER_PAGE = 20;

function Column({ column, tasks, boardId, canEdit, onRefresh, onBoardRefresh, archived, onClickTask, isMobile, allColumns, collapsed: externalCollapsed, onToggleCollapse, tasksLoaded, onFullScreen }) {
  const [dragOver, setDragOver] = useState(false);
  const colTaskCount = tasks.filter(t => t.column_id === column.id).length;
  const [internalCollapsed, setInternalCollapsed] = useState(false);
  // Auto-collapse empty columns on mobile only after tasks are loaded
  const [autoCollapseApplied, setAutoCollapseApplied] = useState(false);
  useEffect(() => {
    if (isMobile && tasksLoaded && !autoCollapseApplied) {
      setAutoCollapseApplied(true);
      if (colTaskCount === 0) setInternalCollapsed(true);
    }
  }, [isMobile, tasksLoaded, colTaskCount, autoCollapseApplied]);
  const collapsed = isMobile ? internalCollapsed : (externalCollapsed || false);
  const toggleCollapse = isMobile ? () => setInternalCollapsed(c => !c) : onToggleCollapse;
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(column.name);
  const [showMenu, setShowMenu] = useState(false);
  const menuRef = useRef(null);
  useEffect(() => {
    if (!showMenu) return;
    const handleClickOutside = (e) => {
      if (menuRef.current && !menuRef.current.contains(e.target)) {
        setShowMenu(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('touchstart', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('touchstart', handleClickOutside);
    };
  }, [showMenu]);
  const [visibleCount, setVisibleCount] = useState(TASKS_PER_PAGE);
  const colTasks = tasks.filter(t => t.column_id === column.id)
    .sort((a, b) => (a.position ?? 999) - (b.position ?? 999));
  const visibleTasks = colTasks.slice(0, visibleCount);
  const hasMore = colTasks.length > visibleCount;

  const handleDrop = async (e) => {
    e.preventDefault();
    setDragOver(false);
    if (!canEdit || archived) return;
    const taskId = e.dataTransfer.getData('taskId');
    if (!taskId) return;
    try {
      await api.moveTask(boardId, taskId, column.id);
      onRefresh();
    } catch (err) {
      if (err.code === 'WIP_LIMIT_EXCEEDED') {
        alert(`WIP limit reached for "${column.name}" (max ${column.wip_limit})`);
      }
    }
  };

  const handleRename = async () => {
    const newName = renameValue.trim();
    if (!newName || newName === column.name) { setRenaming(false); return; }
    try {
      await api.updateColumn(boardId, column.id, { name: newName });
      setRenaming(false);
      onBoardRefresh();
    } catch (err) {
      alert(`Failed to rename: ${err.error || 'Unknown error'}`);
    }
  };

  const handleDelete = async () => {
    if (!confirm(`Delete column "${column.name}"?\n\nNote: Column must be empty (no tasks).`)) return;
    try {
      await api.deleteColumn(boardId, column.id);
      onBoardRefresh();
    } catch (err) {
      alert(err.error || 'Failed to delete column');
    }
  };

  const handleMoveColumn = async (direction) => {
    const sorted = [...allColumns].sort((a, b) => a.position - b.position);
    const idx = sorted.findIndex(c => c.id === column.id);
    const targetIdx = idx + direction;
    if (targetIdx < 0 || targetIdx >= sorted.length) return;
    // Swap positions
    const newOrder = sorted.map(c => c.id);
    [newOrder[idx], newOrder[targetIdx]] = [newOrder[targetIdx], newOrder[idx]];
    try {
      await api.reorderColumns(boardId, newOrder);
      onBoardRefresh();
    } catch (err) {
      alert(err.error || 'Failed to reorder');
    }
  };

  const wipInfo = column.wip_limit
    ? `${colTasks.length}/${column.wip_limit}`
    : `${colTasks.length}`;

  const sortedCols = [...allColumns].sort((a, b) => a.position - b.position);
  const colIdx = sortedCols.findIndex(c => c.id === column.id);
  const isFirst = colIdx === 0;
  const isLast = colIdx === sortedCols.length - 1;

  // Desktop collapsed: render a narrow vertical bar
  if (!isMobile && collapsed) {
    return (
      <div
        style={{
          width: '40px', minWidth: '40px', flex: '0 0 40px',
          background: '#1a2332', borderRadius: '8px', border: '1px solid #334155',
          display: 'flex', flexDirection: 'column', alignItems: 'center',
          cursor: 'pointer', maxHeight: '100%', overflow: 'hidden',
          padding: '8px 0',
        }}
        onClick={toggleCollapse}
        onDragOver={canEdit ? (e) => { e.preventDefault(); toggleCollapse?.(); } : undefined}
        title={`Expand ${column.name}`}
      >
        <span style={{ fontSize: '0.7rem', color: '#94a3b8', marginBottom: '8px' }}>{colTasks.length}</span>
        <span style={{
          writingMode: 'vertical-rl', textOrientation: 'mixed',
          fontSize: '0.8rem', fontWeight: 600, color: '#e2e8f0',
          letterSpacing: '0.5px', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis',
          maxHeight: 'calc(100% - 40px)',
        }}>{column.name}</span>
      </div>
    );
  }

  return (
    <div
      style={styles.column(dragOver && canEdit, isMobile)}
      onDragOver={!isMobile && canEdit ? (e) => { e.preventDefault(); setDragOver(true); } : undefined}
      onDragLeave={!isMobile && canEdit ? () => setDragOver(false) : undefined}
      onDrop={!isMobile && canEdit ? handleDrop : undefined}
    >
      <div
        style={{ ...styles.columnHeader, cursor: 'pointer', position: 'relative' }}
        onClick={!renaming ? toggleCollapse : undefined}
      >
        {renaming ? (
          <input
            autoFocus
            style={{ background: '#1e293b', color: '#e2e8f0', border: '1px solid #3b82f6', borderRadius: '4px', padding: '2px 6px', fontSize: '16px', fontWeight: 600, width: '100%' }}
            value={renameValue}
            onChange={e => setRenameValue(e.target.value)}
            onBlur={handleRename}
            onKeyDown={e => { if (e.key === 'Enter') handleRename(); if (e.key === 'Escape') setRenaming(false); }}
            onClick={e => e.stopPropagation()}
          />
        ) : (
          <span
            onDoubleClick={canEdit && !archived ? (e) => { e.stopPropagation(); setRenameValue(column.name); setRenaming(true); } : undefined}
            title={canEdit ? 'Double-click to rename' : ''}
          >
            {isMobile ? (collapsed ? '▸ ' : '▾ ') : ''}{column.name}
          </span>
        )}
        <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
          <span style={styles.taskCount}>{wipInfo}</span>
          {canEdit && !archived && (
            <span
              style={{ cursor: 'pointer', fontSize: '0.85rem', opacity: 0.6, userSelect: 'none', padding: '0 2px' }}
              onClick={(e) => { e.stopPropagation(); setShowMenu(m => !m); }}
              title="Column options"
            >⚙️</span>
          )}
        </span>
        {showMenu && canEdit && !archived && (
          <div ref={menuRef} style={{
            position: 'absolute', top: '100%', right: 0, zIndex: 50,
            background: '#1e293b', border: '1px solid #334155', borderRadius: '6px',
            padding: '4px 0', minWidth: '140px', boxShadow: '0 4px 12px rgba(0,0,0,.4)',
          }} onClick={e => e.stopPropagation()}>
            {!isMobile && onFullScreen && (
              <button
                style={{ display: 'block', width: '100%', textAlign: 'left', padding: '6px 12px', background: 'none', border: 'none', color: '#e2e8f0', cursor: 'pointer', fontSize: '0.8rem' }}
                onClick={() => { onFullScreen(); setShowMenu(false); }}
                onMouseEnter={e => e.target.style.background = '#334155'}
                onMouseLeave={e => e.target.style.background = 'none'}
              >⛶ Full Screen</button>
            )}
            <button
              style={{ display: 'block', width: '100%', textAlign: 'left', padding: '6px 12px', background: 'none', border: 'none', color: '#e2e8f0', cursor: 'pointer', fontSize: '0.8rem' }}
              onClick={() => { setRenameValue(column.name); setRenaming(true); setShowMenu(false); }}
              onMouseEnter={e => e.target.style.background = '#334155'}
              onMouseLeave={e => e.target.style.background = 'none'}
            >✏️ Rename</button>
            {!isFirst && (
              <button
                style={{ display: 'block', width: '100%', textAlign: 'left', padding: '6px 12px', background: 'none', border: 'none', color: '#e2e8f0', cursor: 'pointer', fontSize: '0.8rem' }}
                onClick={() => { handleMoveColumn(-1); setShowMenu(false); }}
                onMouseEnter={e => e.target.style.background = '#334155'}
                onMouseLeave={e => e.target.style.background = 'none'}
              >{isMobile ? '⬆️ Move Up' : '⬅️ Move Left'}</button>
            )}
            {!isLast && (
              <button
                style={{ display: 'block', width: '100%', textAlign: 'left', padding: '6px 12px', background: 'none', border: 'none', color: '#e2e8f0', cursor: 'pointer', fontSize: '0.8rem' }}
                onClick={() => { handleMoveColumn(1); setShowMenu(false); }}
                onMouseEnter={e => e.target.style.background = '#334155'}
                onMouseLeave={e => e.target.style.background = 'none'}
              >{isMobile ? '⬇️ Move Down' : '➡️ Move Right'}</button>
            )}
            <div style={{ borderTop: '1px solid #334155', margin: '4px 0' }} />
            <button
              style={{ display: 'block', width: '100%', textAlign: 'left', padding: '6px 12px', background: 'none', border: 'none', color: '#ef4444', cursor: 'pointer', fontSize: '0.8rem' }}
              onClick={() => { handleDelete(); setShowMenu(false); }}
              onMouseEnter={e => e.target.style.background = '#334155'}
              onMouseLeave={e => e.target.style.background = 'none'}
            >🗑️ Delete</button>
          </div>
        )}
      </div>
      {!(isMobile && collapsed) && (
        <div style={styles.taskList(isMobile)}>
          {colTasks.length === 0 && (
            <div style={{ ...styles.empty, padding: '16px 10px', fontSize: '0.8rem' }}>
              {canEdit && !isMobile ? 'Drop tasks here' : 'No tasks'}
            </div>
          )}
          {visibleTasks.map(t => (
            <TaskCard
              key={t.id}
              task={t}
              boardId={boardId}
              canEdit={canEdit}
              onRefresh={onRefresh}
              archived={archived}
              onClickTask={onClickTask}
              isMobile={isMobile}
            />
          ))}
          {hasMore && (
            <button
              onClick={() => setVisibleCount(c => c + TASKS_PER_PAGE)}
              style={{
                width: '100%', padding: '8px', margin: '4px 0',
                background: 'rgba(59, 130, 246, 0.1)', border: '1px solid #334155',
                borderRadius: '6px', color: '#94a3b8', cursor: 'pointer',
                fontSize: '0.8rem', textAlign: 'center',
              }}
              onMouseEnter={e => { e.target.style.background = 'rgba(59, 130, 246, 0.2)'; e.target.style.color = '#e2e8f0'; }}
              onMouseLeave={e => { e.target.style.background = 'rgba(59, 130, 246, 0.1)'; e.target.style.color = '#94a3b8'; }}
            >
              Show {Math.min(TASKS_PER_PAGE, colTasks.length - visibleCount)} more ({colTasks.length - visibleCount} remaining)
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function FullScreenColumnView({ column, tasks, boardId, canEdit, onRefresh, onClose, onClickTask, archived }) {
  useEscapeKey(onClose);
  const colTasks = tasks.filter(t => t.column_id === column.id)
    .sort((a, b) => (b.priority || 0) - (a.priority || 0) || (a.title || '').localeCompare(b.title || ''));

  // Responsive grid: up to 3 columns on wide screens
  return (
    <div style={{
      position: 'fixed', top: 0, left: 0, right: 0, bottom: 0,
      background: 'rgba(0,0,0,0.85)', zIndex: 1000,
      display: 'flex', flexDirection: 'column',
      padding: '20px',
    }} onClick={onClose}>
      <div style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        marginBottom: '16px', flexShrink: 0,
      }} onClick={e => e.stopPropagation()}>
        <h2 style={{ margin: 0, color: '#e2e8f0', fontSize: '1.3rem' }}>
          {column.name} <span style={{ color: '#64748b', fontWeight: 400, fontSize: '1rem' }}>({colTasks.length} tasks)</span>
        </h2>
        <button
          onClick={onClose}
          style={{
            background: '#334155', border: 'none', color: '#e2e8f0',
            padding: '6px 14px', borderRadius: '6px', cursor: 'pointer', fontSize: '0.9rem',
          }}
          onMouseEnter={e => e.target.style.background = '#475569'}
          onMouseLeave={e => e.target.style.background = '#334155'}
        >✕ Close</button>
      </div>
      <div style={{
        flex: 1, overflowY: 'auto',
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fill, minmax(300px, 1fr))',
        gap: '10px', alignContent: 'start',
      }} onClick={e => e.stopPropagation()}>
        {colTasks.map(t => (
          <TaskCard
            key={t.id}
            task={t}
            boardId={boardId}
            canEdit={false}
            onRefresh={onRefresh}
            archived={archived}
            onClickTask={onClickTask}
            isMobile={false}
          />
        ))}
        {colTasks.length === 0 && (
          <div style={{ color: '#64748b', padding: '40px', textAlign: 'center', gridColumn: '1 / -1' }}>
            No tasks in this column.
          </div>
        )}
      </div>
    </div>
  );
}

function CreateTaskModal({ boardId, columns, onClose, onCreated, isMobile, allLabels, allAssignees }) {
  const [title, setTitle] = useState('');
  const [desc, setDesc] = useState('');
  const [priority, setPriority] = useState(1);
  const [columnId, setColumnId] = useState(columns[0]?.id || '');
  const [labels, setLabels] = useState('');
  const [assignedTo, setAssignedTo] = useState('');
  const [loading, setLoading] = useState(false);

  // Guard dismiss: only allow backdrop/Esc close when form is empty
  const hasContent = !!(title.trim() || desc.trim() || labels.trim() || assignedTo.trim());
  const safeClose = useCallback(() => { if (!hasContent) onClose(); }, [hasContent, onClose]);
  useEscapeKey(safeClose);

  const submitTask = async () => {
    if ((!title.trim() && !desc.trim()) || loading) return;
    setLoading(true);
    try {
      await api.createTask(boardId, {
        title: title.trim(),
        description: desc.trim() || '',
        priority: Number(priority),
        column_id: columnId,
        labels: normalizeLabels(labels),
        assigned_to: assignedTo.trim() || null,
      });
      onCreated();
      onClose();
    } catch (err) {
      alert(err.error || 'Failed to create task');
    } finally {
      setLoading(false);
    }
  };

  const submit = (e) => { e.preventDefault(); submitTask(); };

  // Ctrl+Enter (Win/Linux) / Cmd+Enter (macOS) submits from anywhere in the modal
  useEffect(() => {
    const handler = (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') { e.preventDefault(); submitTask(); }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  });

  return (
    <div style={styles.modal(isMobile)} onClick={safeClose}>
      <div style={styles.modalContent(isMobile)} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ marginBottom: '16px', color: '#f1f5f9' }}>New Task</h3>
        <form onSubmit={submit}>
          <input style={styles.input} placeholder="Title (optional if description provided)" value={title} onChange={e => setTitle(e.target.value)} autoFocus />
          <textarea style={styles.textarea} placeholder="Description (optional if title provided)" value={desc} onChange={e => setDesc(e.target.value)} />
          <div style={{ display: 'flex', gap: '10px', marginBottom: '10px', alignItems: 'stretch' }}>
            <PriorityToggle value={priority} onChange={setPriority} compact={isMobile} />
            <StyledSelect style={{ ...styles.select, marginBottom: 0 }} value={columnId} onChange={e => setColumnId(e.target.value)}>
              {columns.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
            </StyledSelect>
          </div>
          <AutocompleteInput style={styles.input} placeholder="Labels (comma-separated)" value={labels} onChange={setLabels} suggestions={allLabels || []} isCommaList />
          {(allLabels || []).length > 0 && (
            <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap', marginTop: '-6px', marginBottom: '6px' }}>
              {(allLabels || []).slice(0, 8).map(l => {
                const current = labels.split(',').map(s => s.trim()).filter(Boolean);
                const isActive = current.includes(l);
                return (
                  <button key={l} type="button" onClick={() => {
                    if (isActive) {
                      setLabels(current.filter(c => c !== l).join(', '));
                    } else {
                      setLabels(current.length ? [...current, l].join(', ') : l);
                    }
                  }} style={{
                    padding: '2px 8px', fontSize: '0.7rem', borderRadius: '10px', cursor: 'pointer',
                    background: isActive ? '#3b82f633' : '#1e293b', color: isActive ? '#93c5fd' : '#64748b',
                    border: `1px solid ${isActive ? '#3b82f644' : '#334155'}`, whiteSpace: 'nowrap',
                  }}>{l}</button>
                );
              })}
            </div>
          )}
          <AutocompleteInput style={styles.input} placeholder="Assigned to (optional)" value={assignedTo} onChange={setAssignedTo} suggestions={allAssignees || []} />
          {(allAssignees || []).length > 0 && (
            <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap', marginTop: '-6px', marginBottom: '6px' }}>
              {(allAssignees || []).slice(0, 8).map(a => {
                const isActive = assignedTo.trim() === a;
                return (
                  <button key={a} type="button" onClick={() => {
                    setAssignedTo(isActive ? '' : a);
                  }} style={{
                    padding: '2px 8px', fontSize: '0.7rem', borderRadius: '10px', cursor: 'pointer',
                    background: isActive ? '#22c55e33' : '#1e293b', color: isActive ? '#86efac' : '#64748b',
                    border: `1px solid ${isActive ? '#22c55e44' : '#334155'}`, whiteSpace: 'nowrap',
                  }}>{a}</button>
                );
              })}
            </div>
          )}
          <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
            <button type="button" style={styles.btn('secondary', isMobile)} onClick={onClose}>Cancel</button>
            <button type="submit" style={styles.btn('primary', isMobile)} disabled={loading || (!title.trim() && !desc.trim())}>
              {loading ? 'Creating...' : 'Create Task'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

function TaskDetailModal({ boardId, task, canEdit, onClose, onRefresh, isMobile, allColumns, allLabels, allAssignees, quickDoneColumnId, quickDoneAutoArchive, quickReassignColumnId, quickReassignTo }) {
  const [events, setEvents] = useState([]);
  const [comment, setComment] = useState('');
  const [actorName, setActorName] = useState(() => api.getDisplayName());
  const [loadingEvents, setLoadingEvents] = useState(true);
  const commentsEndRef = useRef(null);
  const [posting, setPosting] = useState(false);
  const [showMove, setShowMove] = useState(false);
  const [markingDone, setMarkingDone] = useState(false);
  const [reassigning, setReassigning] = useState(false);
  const [editing, setEditing] = useState(false);
  const [editTitle, setEditTitle] = useState(task.title || '');
  const [editDesc, setEditDesc] = useState(task.description || '');
  const [editPriority, setEditPriority] = useState(task.priority);
  const [editLabels, setEditLabels] = useState((task.labels || []).join(', '));
  const [editAssigned, setEditAssigned] = useState(task.assigned_to || '');
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [archiving, setArchiving] = useState(false);
  const isArchived = !!task.archived_at;

  // Guard dismiss: don't allow backdrop/Esc close when editing or comment in progress
  const hasUnsaved = editing || !!(comment.trim());
  const safeClose = useCallback(() => { if (!hasUnsaved) onClose(); }, [hasUnsaved, onClose]);
  useEscapeKey(safeClose);

  const handleArchiveToggle = async () => {
    setArchiving(true);
    try {
      if (isArchived) {
        await api.unarchiveTask(boardId, task.id);
      } else {
        await api.archiveTask(boardId, task.id);
      }
      onRefresh();
      onClose();
    } catch (err) {
      alert(err.error || 'Failed to archive/unarchive task');
    } finally {
      setArchiving(false);
    }
  };

  // Determine the done column: configured or last column
  const doneColumn = (() => {
    if (!allColumns || allColumns.length === 0) return null;
    if (quickDoneColumnId) {
      return allColumns.find(c => c.id === quickDoneColumnId) || null;
    }
    // Default: last column by position
    return allColumns.reduce((a, b) => (a.position > b.position ? a : b), allColumns[0]);
  })();

  const isAlreadyDone = doneColumn && task.column_id === doneColumn.id;

  const handleMarkDone = async () => {
    if (!doneColumn || isAlreadyDone) return;
    setMarkingDone(true);
    try {
      await api.moveTask(boardId, task.id, doneColumn.id);
      if (quickDoneAutoArchive) {
        await api.archiveTask(boardId, task.id);
      }
      onRefresh();
      onClose();
    } catch (err) {
      alert(err.error || 'Failed to mark task as done');
    } finally {
      setMarkingDone(false);
    }
  };

  // Determine the reassign column: configured or first column
  const reassignColumn = (() => {
    if (!allColumns || allColumns.length === 0 || !quickReassignColumnId) return null;
    return allColumns.find(c => c.id === quickReassignColumnId) || null;
  })();

  const isAlreadyInReassignCol = reassignColumn && task.column_id === reassignColumn.id;

  const handleQuickReassign = async () => {
    if (!reassignColumn || isAlreadyInReassignCol) return;
    setReassigning(true);
    try {
      // Move to target column
      await api.moveTask(boardId, task.id, reassignColumn.id);
      // Optionally set assigned_to
      if (quickReassignTo) {
        await api.updateTask(boardId, task.id, { assigned_to: quickReassignTo });
      }
      onRefresh();
      onClose();
    } catch (err) {
      alert(err.error || 'Failed to reassign task');
    } finally {
      setReassigning(false);
    }
  };

  const loadEvents = useCallback(async () => {
    try {
      const { data } = await api.getTaskEvents(boardId, task.id);
      setEvents(data || []);
    } catch (err) {
      console.error('Failed to load events:', err);
    } finally {
      setLoadingEvents(false);
    }
  }, [boardId, task.id]);

  useEffect(() => { loadEvents(); }, [loadEvents]);

  const submitComment = async (e) => {
    e.preventDefault();
    if (!comment.trim()) return;
    setPosting(true);
    try {
      const nameToUse = actorName.trim() || undefined;
      // Persist the name for future use
      if (nameToUse) api.setDisplayName(nameToUse);
      await api.commentOnTask(boardId, task.id, comment.trim(), nameToUse);
      setComment('');
      loadEvents();
    } catch (err) {
      alert(err.error || 'Failed to post comment');
    } finally {
      setPosting(false);
    }
  };

  const saveEdit = async () => {
    if (!editTitle.trim() && !editDesc.trim()) {
      alert('Either title or description must be provided');
      return;
    }
    setSaving(true);
    try {
      const updates = {};
      if (editTitle.trim() !== (task.title || '')) updates.title = editTitle.trim();
      if (editDesc.trim() !== (task.description || '')) updates.description = editDesc.trim();
      if (editPriority !== task.priority) updates.priority = editPriority;
      const newLabels = normalizeLabels(editLabels);
      const oldLabels = task.labels || [];
      if (JSON.stringify(newLabels) !== JSON.stringify(oldLabels)) updates.labels = newLabels;
      if ((editAssigned.trim() || null) !== (task.assigned_to || null)) updates.assigned_to = editAssigned.trim() || null;

      if (Object.keys(updates).length > 0) {
        await api.updateTask(boardId, task.id, updates);
        onRefresh();
      }
      setEditing(false);
    } catch (err) {
      alert(err.error || 'Failed to update task');
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    if (!confirm('Delete this task? This cannot be undone.')) return;
    setDeleting(true);
    try {
      await api.deleteTask(boardId, task.id);
      onRefresh();
      onClose();
    } catch (err) {
      alert(err.error || 'Failed to delete task');
    } finally {
      setDeleting(false);
    }
  };

  const comments = events.filter(e => e.event_type === 'comment');
  const activity = events.filter(e => e.event_type !== 'comment');

  // Auto-scroll comments to bottom when new comments are added
  useEffect(() => {
    if (commentsEndRef.current) {
      commentsEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [comments.length]);

  const formatTime = (ts) => {
    try {
      const d = parseUTC(ts);
      return d.toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
    } catch { return ts; }
  };

  const eventLabel = (evt) => {
    switch (evt.event_type) {
      case 'created': return '🆕 Created';
      case 'moved': return `➡️ Moved to ${evt.data?.to_column || 'column'}`;
      case 'claimed': return `🔒 Claimed`;
      case 'released': return '🔓 Released';
      case 'updated': return '✏️ Updated';
      case 'assigned': return `👤 Assigned to ${evt.data?.assigned_to || 'someone'}`;
      case 'archived': return '📦 Archived';
      case 'unarchived': return '📤 Unarchived';
      case 'deleted': return '🗑️ Deleted';
      default: return evt.event_type;
    }
  };

  return (
    <div style={styles.modal(isMobile)} onClick={safeClose}>
      <div style={styles.modalContentWide(isMobile)} onClick={(e) => e.stopPropagation()}>
        {/* Task header */}
        <div style={{ marginBottom: '16px' }}>
          {/* Row 1: Title + Close (mobile) or Title + all buttons (desktop) */}
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
            <div style={{ flex: 1, minWidth: 0 }}>
              {editing ? (
                <input
                  style={{ ...styles.input, fontSize: '1.1rem', fontWeight: 600, marginBottom: '6px' }}
                  value={editTitle}
                  onChange={e => setEditTitle(e.target.value)}
                  autoFocus
                />
              ) : (
                <h3 style={{ color: task.title ? '#f1f5f9' : '#94a3b8', marginBottom: '6px', fontSize: isMobile ? '1rem' : '1.17rem' }}>{task.title || (task.description ? task.description.slice(0, 80) + (task.description.length > 80 ? '…' : '') : '(untitled)')}</h3>
              )}
              {!editing && (
                <div style={styles.cardMeta}>
                  <span style={{ color: priorityColor(task.priority) }}>
                    {priorityLabel(task.priority)}
                  </span>
                  {task.assigned_to && <span>→ {task.assigned_to}</span>}
                  {task.claimed_by && <span>🔒 {task.claimed_by}</span>}
                  {task.column_name && <span>in {task.column_name}</span>}
                </div>
              )}
            </div>
            {/* Desktop: all buttons inline; Mobile: just close */}
            {!isMobile ? (
              <div style={{ display: 'flex', gap: '4px', marginLeft: '8px', flexShrink: 0 }}>
                {canEdit && !editing && reassignColumn && !isAlreadyInReassignCol && !isArchived && (
                  <button
                    style={{ ...styles.btnIcon, background: '#f59e0b22', borderColor: '#f59e0b44', color: '#fbbf24' }}
                    onClick={handleQuickReassign}
                    disabled={reassigning}
                    title={`Move to ${reassignColumn.name}${quickReassignTo ? ` → ${quickReassignTo}` : ''}`}
                  >{reassigning ? '⏳' : '↩'}</button>
                )}
                {canEdit && !editing && doneColumn && !isAlreadyDone && !isArchived && (
                  <button
                    style={{ ...styles.btnIcon, background: '#22c55e22', borderColor: '#22c55e44', color: '#4ade80' }}
                    onClick={handleMarkDone}
                    disabled={markingDone}
                    title={`Mark done${quickDoneAutoArchive ? ' & archive' : ''} → ${doneColumn.name}`}
                  >{markingDone ? '⏳' : '✓'}</button>
                )}
                {canEdit && !editing && (
                  <button
                    style={styles.btnIcon}
                    onClick={handleArchiveToggle}
                    disabled={archiving}
                    title={isArchived ? 'Unarchive task' : 'Archive task'}
                  >{archiving ? '⏳' : isArchived ? '📤' : '📦'}</button>
                )}
                {canEdit && !editing && (
                  <button
                    style={styles.btnIcon}
                    onClick={() => setEditing(true)}
                    title="Edit task"
                  >✏️</button>
                )}
                <button style={styles.btnClose} onClick={onClose}>×</button>
              </div>
            ) : (
              <button style={{ ...styles.btnClose, marginLeft: '8px', flexShrink: 0 }} onClick={onClose}>×</button>
            )}
          </div>
          {/* Row 2: Action buttons on mobile (below title) */}
          {isMobile && canEdit && !editing && (
            <div style={{ display: 'flex', gap: '6px', justifyContent: 'flex-end', marginTop: '10px', flexWrap: 'wrap' }}>
              {reassignColumn && !isAlreadyInReassignCol && !isArchived && (
                <button
                  style={{ ...styles.btnIcon, background: '#f59e0b22', borderColor: '#f59e0b44', color: '#fbbf24' }}
                  onClick={handleQuickReassign}
                  disabled={reassigning}
                  title={`Move to ${reassignColumn.name}${quickReassignTo ? ` → ${quickReassignTo}` : ''}`}
                >{reassigning ? '⏳' : '↩'}</button>
              )}
              {doneColumn && !isAlreadyDone && !isArchived && (
                <button
                  style={{ ...styles.btnIcon, background: '#22c55e22', borderColor: '#22c55e44', color: '#4ade80' }}
                  onClick={handleMarkDone}
                  disabled={markingDone}
                  title={`Mark done${quickDoneAutoArchive ? ' & archive' : ''} → ${doneColumn.name}`}
                >{markingDone ? '⏳' : '✓'}</button>
              )}
              <button
                style={styles.btnIcon}
                onClick={handleArchiveToggle}
                disabled={archiving}
                title={isArchived ? 'Unarchive task' : 'Archive task'}
              >{archiving ? '⏳' : isArchived ? '📤' : '📦'}</button>
              <button
                style={styles.btnIcon}
                onClick={() => setEditing(true)}
                title="Edit task"
              >✏️</button>
            </div>
          )}
        </div>

        {/* Edit form */}
        {editing && (
          <div style={{ marginBottom: '16px', padding: '12px', background: '#0f172a', borderRadius: '6px', border: '1px solid #6366f133' }}>
            <textarea
              ref={el => { if (el) { el.style.height = 'auto'; el.style.height = Math.max(140, el.scrollHeight) + 'px'; } }}
              style={{ ...styles.textarea, minHeight: '140px', overflow: 'hidden' }}
              placeholder="Description (optional)"
              value={editDesc}
              onChange={e => {
                setEditDesc(e.target.value);
                e.target.style.height = 'auto';
                e.target.style.height = Math.max(140, e.target.scrollHeight) + 'px';
              }}
            />
            <div style={{ display: 'flex', gap: '10px', marginBottom: '10px' }}>
              <PriorityToggle value={editPriority} onChange={setEditPriority} compact={isMobile} />
            </div>
            <AutocompleteInput
              style={styles.input}
              placeholder="Labels (comma-separated)"
              value={editLabels}
              onChange={setEditLabels}
              suggestions={allLabels || []}
              isCommaList
            />
            {(allLabels || []).length > 0 && (
              <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap', marginTop: '-6px', marginBottom: '6px' }}>
                {(allLabels || []).slice(0, 8).map(l => {
                  const current = editLabels.split(',').map(s => s.trim()).filter(Boolean);
                  const isActive = current.includes(l);
                  return (
                    <button key={l} type="button" onClick={() => {
                      if (isActive) {
                        setEditLabels(current.filter(c => c !== l).join(', '));
                      } else {
                        setEditLabels(current.length ? [...current, l].join(', ') : l);
                      }
                    }} style={{
                      padding: '2px 8px', fontSize: '0.7rem', borderRadius: '10px', cursor: 'pointer',
                      background: isActive ? '#3b82f633' : '#1e293b', color: isActive ? '#93c5fd' : '#64748b',
                      border: `1px solid ${isActive ? '#3b82f644' : '#334155'}`, whiteSpace: 'nowrap',
                    }}>{l}</button>
                  );
                })}
              </div>
            )}
            <AutocompleteInput
              style={styles.input}
              placeholder="Assigned to (optional)"
              value={editAssigned}
              onChange={setEditAssigned}
              suggestions={allAssignees || []}
            />
            {(allAssignees || []).length > 0 && (
              <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap', marginTop: '-6px', marginBottom: '6px' }}>
                {(allAssignees || []).slice(0, 8).map(a => {
                  const isActive = editAssigned.trim() === a;
                  return (
                    <button key={a} type="button" onClick={() => {
                      setEditAssigned(isActive ? '' : a);
                    }} style={{
                      padding: '2px 8px', fontSize: '0.7rem', borderRadius: '10px', cursor: 'pointer',
                      background: isActive ? '#22c55e33' : '#1e293b', color: isActive ? '#86efac' : '#64748b',
                      border: `1px solid ${isActive ? '#22c55e44' : '#334155'}`, whiteSpace: 'nowrap',
                    }}>{a}</button>
                  );
                })}
              </div>
            )}
            <div style={{ display: 'flex', gap: '8px', justifyContent: 'space-between' }}>
              <button
                style={styles.btn('danger', isMobile)}
                onClick={handleDelete}
                disabled={deleting}
              >
                {deleting ? 'Deleting...' : '🗑️ Delete'}
              </button>
              <div style={{ display: 'flex', gap: '8px' }}>
                <button style={styles.btn('secondary', isMobile)} onClick={() => setEditing(false)}>Cancel</button>
                <button
                  style={styles.btn('primary', isMobile)}
                  onClick={saveEdit}
                  disabled={saving || (!editTitle.trim() && !editDesc.trim())}
                >
                  {saving ? 'Saving...' : 'Save'}
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Mobile move action */}
        {canEdit && allColumns && !editing && (
          <div style={{ marginBottom: '12px' }}>
            {showMove ? (
              <MoveTaskDropdown
                boardId={boardId}
                task={task}
                columns={allColumns}
                onMoved={() => { setShowMove(false); onRefresh(); onClose(); }}
                onCancel={() => setShowMove(false)}
              />
            ) : (
              <button style={{ ...styles.btnSmall, padding: '6px 10px', width: '100%' }} onClick={() => setShowMove(true)}>
                ➡️ Move to column...
              </button>
            )}
          </div>
        )}

        {/* Description (view mode) */}
        {!editing && task.description && (
          <div style={{ marginBottom: '16px', padding: '10px 12px', background: '#0f172a', borderRadius: '6px', border: '1px solid #334155' }}>
            <div style={{ fontSize: '0.73rem', color: '#64748b', marginBottom: '4px', textTransform: 'uppercase', fontWeight: 600 }}>Description</div>
            <div style={{ color: '#cbd5e1', fontSize: '0.85rem', whiteSpace: 'pre-wrap' }}>{task.description}</div>
          </div>
        )}

        {/* Labels (view mode) */}
        {!editing && task.labels && task.labels.length > 0 && (
          <div style={{ display: 'flex', gap: '4px', marginBottom: '16px', flexWrap: 'wrap' }}>
            {task.labels.map((l, i) => <span key={i} style={styles.label()}>{l}</span>)}
          </div>
        )}

        {/* Comments section */}
        <div style={{ borderTop: '1px solid #334155', paddingTop: '14px' }}>
          <div style={{ fontSize: '0.8rem', fontWeight: 600, color: '#94a3b8', marginBottom: '10px' }}>
            💬 Comments ({comments.length})
          </div>

          {loadingEvents ? (
            <div style={{ color: '#475569', fontSize: '0.8rem', padding: '10px 0' }}>Loading...</div>
          ) : comments.length === 0 ? (
            <div style={{ color: '#475569', fontSize: '0.8rem', padding: '10px 0' }}>No comments yet.</div>
          ) : (
            <div style={{ maxHeight: isMobile ? '30vh' : '40vh', overflowY: 'auto', marginBottom: '12px' }}>
              {comments.map(evt => (
                <div key={evt.id} style={{ marginBottom: '10px', padding: '8px 10px', background: '#0f172a', borderRadius: '6px', border: '1px solid #334155' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '4px' }}>
                    <span style={{ fontSize: '0.78rem', fontWeight: 600, color: '#a5b4fc' }}>{evt.actor || 'anonymous'}</span>
                    <span style={{ fontSize: '0.7rem', color: '#475569' }}>{formatTime(evt.created_at)}</span>
                  </div>
                  <div style={{ fontSize: '0.83rem', color: '#cbd5e1', whiteSpace: 'pre-wrap' }}>
                    {renderWithMentions(evt.data?.message || '')}
                  </div>
                </div>
              ))}
              <div ref={commentsEndRef} />
            </div>
          )}

          {/* Add comment form */}
          {canEdit && (
            <form onSubmit={submitComment} style={{ marginTop: '8px' }}>
              <input
                style={styles.input}
                placeholder="Your name (optional)"
                value={actorName}
                onChange={e => setActorName(e.target.value)}
              />
              <textarea
                style={{ ...styles.textarea, minHeight: '50px' }}
                placeholder="Add a comment..."
                value={comment}
                onChange={e => setComment(e.target.value)}
                onKeyDown={e => {
                  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
                    e.preventDefault();
                    if (comment.trim() && !posting) {
                      submitComment(e);
                    }
                  }
                }}
              />
              <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
                <button
                  type="submit"
                  style={styles.btn('primary', isMobile)}
                  disabled={posting || !comment.trim()}
                >
                  {posting ? 'Posting...' : 'Comment'}
                </button>
              </div>
            </form>
          )}
        </div>

        {/* Activity log */}
        {activity.length > 0 && (
          <details style={{ marginTop: '12px', borderTop: '1px solid #334155', paddingTop: '10px' }}>
            <summary style={{ fontSize: '0.75rem', color: '#64748b', cursor: 'pointer', userSelect: 'none' }}>
              📜 Activity ({activity.length} events)
            </summary>
            <div style={{ maxHeight: '160px', overflowY: 'auto', marginTop: '8px' }}>
              {activity.map(evt => (
                <div key={evt.id} style={{ fontSize: '0.75rem', color: '#64748b', padding: '3px 0', display: 'flex', justifyContent: 'space-between' }}>
                  <span>{eventLabel(evt)} {evt.actor ? `by ${evt.actor}` : ''}</span>
                  <span style={{ fontSize: '0.68rem', color: '#475569' }}>{formatTime(evt.created_at)}</span>
                </div>
              ))}
            </div>
          </details>
        )}
      </div>
    </div>
  );
}

function CreateBoardModal({ onClose, onCreated, isMobile }) {
  const [name, setName] = useState('');
  const [desc, setDesc] = useState('');
  const [isPublic, setIsPublic] = useState(false);
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState(null);
  const [copied, setCopied] = useState(null);

  // Guard dismiss: only allow backdrop/Esc close when form is empty
  const hasContent = !!(name.trim() || desc.trim());
  const safeClose = useCallback(() => { if (!hasContent) onClose(); }, [hasContent, onClose]);
  useEscapeKey(result ? onClose : safeClose);

  const submit = async (e) => {
    e.preventDefault();
    if (!name.trim()) return;
    setLoading(true);
    try {
      const { data } = await api.createBoard({
        name: name.trim(),
        description: desc.trim() || undefined,
        is_public: isPublic,
      });
      setResult(data);
    } catch (err) {
      alert(err.error || 'Failed to create board');
    } finally {
      setLoading(false);
    }
  };

  const handleCopy = (text, label) => {
    copyToClipboard(text);
    setCopied(label);
    setTimeout(() => setCopied(null), 2000);
  };

  const handleDone = () => {
    onCreated(result?.board_id);
    onClose();
  };

  if (result) {
    const origin = window.location.origin;
    const viewUrl = `${origin}/board/${result.board_id}`;
    const manageUrl = `${origin}/board/${result.board_id}?key=${result.manage_key}`;

    return (
      <div style={styles.modal(isMobile)} onClick={handleDone}>
        <div style={styles.modalContent(isMobile)} onClick={(e) => e.stopPropagation()}>
          <div style={styles.successBox}>
            <h3 style={{ color: '#22c55e', marginBottom: '8px', fontSize: isMobile ? '1rem' : '1.17rem' }}>✅ Board Created!</h3>
            <p style={{ color: '#94a3b8', fontSize: '0.85rem' }}>
              Save your management link — it's the only way to edit this board.
            </p>
          </div>

          <div style={{ marginBottom: '14px' }}>
            <div style={styles.urlLabel}>🔗 View Link (read-only)</div>
            <div style={styles.urlBox}>
              <span style={{ flex: 1, minWidth: 0, overflow: 'hidden' }}>{viewUrl}</span>
              <button style={{ ...styles.btnSmall, flexShrink: 0 }} onClick={() => handleCopy(viewUrl, 'view')}>
                {copied === 'view' ? '✓' : 'Copy'}
              </button>
            </div>
          </div>

          <div style={{ marginBottom: '14px' }}>
            <div style={styles.urlLabel}>🔑 Manage Link (keep private!)</div>
            <div style={{ ...styles.urlBox, borderColor: '#6366f155' }}>
              <span style={{ flex: 1, minWidth: 0, overflow: 'hidden', color: '#a5b4fc' }}>{manageUrl}</span>
              <button style={{ ...styles.btnSmall, borderColor: '#6366f1', color: '#a5b4fc', flexShrink: 0 }} onClick={() => handleCopy(manageUrl, 'manage')}>
                {copied === 'manage' ? '✓' : 'Copy'}
              </button>
            </div>
          </div>

          <div style={{ marginBottom: '14px' }}>
            <div style={styles.urlLabel}>🤖 API Base</div>
            <div style={styles.urlBox}>
              <span style={{ flex: 1, minWidth: 0, overflow: 'hidden' }}>{origin}{result.api_base}</span>
              <button style={{ ...styles.btnSmall, flexShrink: 0 }} onClick={() => handleCopy(`${origin}${result.api_base}`, 'api')}>
                {copied === 'api' ? '✓' : 'Copy'}
              </button>
            </div>
            <p style={{ fontSize: '0.73rem', color: '#64748b', marginTop: '4px' }}>
              Use <code style={{ color: '#94a3b8' }}>Authorization: Bearer {'{manage_key}'}</code> for write ops.
            </p>
          </div>

          <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
            <button style={styles.btn('primary', isMobile)} onClick={handleDone}>
              Open Board →
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div style={styles.modal(isMobile)} onClick={safeClose}>
      <div style={styles.modalContent(isMobile)} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ marginBottom: '16px', color: '#f1f5f9' }}>New Board</h3>
        <form onSubmit={submit}>
          <input style={styles.input} placeholder="Board Name" value={name} onChange={e => setName(e.target.value)} autoFocus />
          <textarea style={styles.textarea} placeholder="Description (optional)" value={desc} onChange={e => setDesc(e.target.value)} />
          {/* Boards are created with default columns on the server. */}
          <label style={{ fontSize: '0.85rem', color: '#94a3b8', cursor: 'pointer', marginBottom: '12px', display: 'flex', alignItems: 'center', gap: '8px' }}>
            <input type="checkbox" checked={isPublic} onChange={e => setIsPublic(e.target.checked)} />
            Make board public
          </label>
          <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end', marginTop: '12px' }}>
            <button type="button" style={styles.btn('secondary', isMobile)} onClick={onClose}>Cancel</button>
            <button type="submit" style={styles.btn('primary', isMobile)} disabled={loading || !name.trim()}>
              {loading ? 'Creating...' : 'Create Board'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ---- Webhook Manager ----
const WEBHOOK_EVENTS = [
  'task.created', 'task.updated', 'task.deleted',
  'task.moved', 'task.claimed', 'task.released', 'task.comment',
];

function BoardSettingsModal({ board, canEdit, onClose, onRefresh, onBoardListRefresh, isMobile }) {
  const [name, setName] = useState(board.name);
  const [description, setDescription] = useState(board.description || '');
  const [isPublic, setIsPublic] = useState(board.is_public || false);
  const [requireDisplayName, setRequireDisplayName] = useState(board.require_display_name || false);
  const [quickDoneColumnId, setQuickDoneColumnId] = useState(board.quick_done_column_id || '');
  const [quickDoneAutoArchive, setQuickDoneAutoArchive] = useState(board.quick_done_auto_archive || false);
  const [quickReassignColumnId, setQuickReassignColumnId] = useState(board.quick_reassign_column_id || '');
  const [quickReassignTo, setQuickReassignTo] = useState(board.quick_reassign_to || '');
  const [saving, setSaving] = useState(false);
  const [showWebhooks, setShowWebhooks] = useState(false);
  const [archiving, setArchiving] = useState(false);
  const [confirmArchive, setConfirmArchive] = useState(false);
  const [error, setError] = useState('');
  const isArchived = !!board.archived_at;

  // Guard dismiss: only allow backdrop/Esc close when no unsaved changes
  const hasChanges = name !== board.name || description !== (board.description || '') ||
    isPublic !== (board.is_public || false) || requireDisplayName !== (board.require_display_name || false) ||
    quickDoneColumnId !== (board.quick_done_column_id || '') || quickDoneAutoArchive !== (board.quick_done_auto_archive || false) ||
    quickReassignColumnId !== (board.quick_reassign_column_id || '') || quickReassignTo !== (board.quick_reassign_to || '');
  const safeClose = useCallback(() => { if (!hasChanges) onClose(); }, [hasChanges, onClose]);
  useEscapeKey(safeClose);

  const handleSave = async () => {
    setError('');
    if (!name.trim()) { setError('Board name is required'); return; }
    setSaving(true);
    try {
      await api.updateBoard(board.id, {
        name: name.trim(),
        description: description.trim(),
        is_public: isPublic,
        require_display_name: requireDisplayName,
        quick_done_column_id: quickDoneColumnId || '',
        quick_done_auto_archive: quickDoneAutoArchive,
        quick_reassign_column_id: quickReassignColumnId || '',
        quick_reassign_to: quickReassignTo.trim() || '',
      });
      onRefresh();
      onClose();
    } catch (err) {
      setError(err.error || 'Failed to update board');
    } finally {
      setSaving(false);
    }
  };

  const handleArchiveToggle = async () => {
    if (!isArchived && !confirmArchive) {
      setConfirmArchive(true);
      return;
    }
    setArchiving(true);
    setError('');
    try {
      if (isArchived) {
        await api.unarchiveBoard(board.id);
      } else {
        await api.archiveBoard(board.id);
      }
      onRefresh();
      if (onBoardListRefresh) onBoardListRefresh();
      onClose();
    } catch (err) {
      setError(err.error || `Failed to ${isArchived ? 'unarchive' : 'archive'} board`);
    } finally {
      setArchiving(false);
      setConfirmArchive(false);
    }
  };

  return (
    <div style={styles.modal(isMobile)} onClick={safeClose}>
      <div style={styles.modalContent(isMobile)} onClick={e => e.stopPropagation()}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
          <h2 style={{ color: '#f1f5f9', fontSize: '1.1rem', margin: 0 }}>⚙️ Board Settings</h2>
          <button style={styles.btnClose} onClick={onClose}>×</button>
        </div>

        {error && (
          <div style={{ background: '#ef444422', border: '1px solid #ef444444', borderRadius: '4px', padding: '8px 12px', marginBottom: '12px', color: '#fca5a5', fontSize: '0.8rem' }}>
            {error}
          </div>
        )}

        <label style={{ color: '#94a3b8', fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Name</label>
        <input
          style={styles.input}
          value={name}
          onChange={e => setName(e.target.value)}
          disabled={!canEdit}
        />

        <label style={{ color: '#94a3b8', fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Description</label>
        <textarea
          style={styles.textarea}
          value={description}
          onChange={e => setDescription(e.target.value)}
          disabled={!canEdit}
        />

        <label style={{ color: '#94a3b8', fontSize: '0.8rem', display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px', cursor: canEdit ? 'pointer' : 'default' }}>
          <input
            type="checkbox"
            checked={isPublic}
            onChange={e => setIsPublic(e.target.checked)}
            disabled={!canEdit}
          />
          Public (listed in board directory)
        </label>

        <label style={{ color: '#94a3b8', fontSize: '0.8rem', display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '16px', cursor: canEdit ? 'pointer' : 'default' }}>
          <input
            type="checkbox"
            checked={requireDisplayName}
            onChange={e => setRequireDisplayName(e.target.checked)}
            disabled={!canEdit}
          />
          Require display name (no anonymous tasks or comments)
        </label>

        {canEdit && (
          <div style={{ borderTop: '1px solid #334155', paddingTop: '12px', marginBottom: '16px' }}>
            <label style={{ color: '#94a3b8', fontSize: '0.8rem', display: 'block', marginBottom: '8px', fontWeight: 600 }}>Quick Done Button <span style={{ color: '#22c55e' }}>✓</span></label>
            <label style={{ color: '#94a3b8', fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Target column</label>
            <StyledSelect
              style={{ ...styles.input, cursor: 'pointer' }}
              value={quickDoneColumnId}
              onChange={e => setQuickDoneColumnId(e.target.value)}
            >
              <option value="">Last column (default)</option>
              {(board.columns || []).map(col => (
                <option key={col.id} value={col.id}>{col.name}</option>
              ))}
            </StyledSelect>
            <label style={{ color: '#94a3b8', fontSize: '0.8rem', display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '12px', cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={quickDoneAutoArchive}
                onChange={e => setQuickDoneAutoArchive(e.target.checked)}
              />
              Auto-archive task when marked done
            </label>
          </div>
        )}

        {canEdit && (
          <div style={{ borderTop: '1px solid #334155', paddingTop: '12px', marginBottom: '16px' }}>
            <label style={{ color: '#94a3b8', fontSize: '0.8rem', display: 'block', marginBottom: '8px', fontWeight: 600 }}>Quick Reassign Button <span style={{ color: '#f59e0b' }}>↩</span></label>
            <label style={{ color: '#94a3b8', fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Target column</label>
            <StyledSelect
              style={{ ...styles.input, cursor: 'pointer' }}
              value={quickReassignColumnId}
              onChange={e => setQuickReassignColumnId(e.target.value)}
            >
              <option value="">Disabled (no button shown)</option>
              {(board.columns || []).map(col => (
                <option key={col.id} value={col.id}>{col.name}</option>
              ))}
            </StyledSelect>
            <label style={{ color: '#94a3b8', fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Assign to (optional)</label>
            <input
              style={styles.input}
              value={quickReassignTo}
              onChange={e => setQuickReassignTo(e.target.value)}
              placeholder="e.g. Jordan, Nanook"
            />
          </div>
        )}

        {canEdit && (
          <div style={{ borderTop: '1px solid #334155', paddingTop: '12px', marginBottom: '16px' }}>
            <button
              onClick={() => setShowWebhooks(true)}
              style={{
                ...styles.btn('secondary', isMobile),
                width: '100%',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                gap: '6px',
              }}
            >
              ⚡ Manage Webhooks
            </button>
          </div>
        )}

        <div style={{ color: '#64748b', fontSize: '0.75rem', marginBottom: '16px' }}>
          <div>Board ID: <code style={{ color: '#94a3b8' }}>{board.id}</code></div>
          <div>Created: {parseUTC(board.created_at).toLocaleString()}</div>
          {isArchived && <div style={{ color: '#f59e0b', marginTop: '4px' }}>📦 This board is archived</div>}
        </div>

        {canEdit && (
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
            {confirmArchive ? (
              <div style={{ display: 'flex', gap: '6px', alignItems: 'center' }}>
                <span style={{ color: '#f59e0b', fontSize: '0.75rem' }}>Archive this board?</span>
                <button
                  style={{ ...styles.btn('danger', isMobile), fontSize: '0.75rem', padding: '4px 10px' }}
                  onClick={handleArchiveToggle}
                  disabled={archiving}
                >
                  {archiving ? '...' : 'Yes, archive'}
                </button>
                <button
                  style={{ ...styles.btn('secondary', isMobile), fontSize: '0.75rem', padding: '4px 10px' }}
                  onClick={() => setConfirmArchive(false)}
                >
                  Cancel
                </button>
              </div>
            ) : (
              <button
                style={{
                  ...styles.btn(isArchived ? 'primary' : 'secondary', isMobile),
                  fontSize: '0.75rem',
                  ...(isArchived ? {} : { color: '#f59e0b', borderColor: '#f59e0b44' }),
                }}
                onClick={handleArchiveToggle}
                disabled={archiving}
              >
                {archiving ? '...' : isArchived ? '📤 Unarchive Board' : '📦 Archive Board'}
              </button>
            )}
            <button
              style={{ ...styles.btn('primary', isMobile), marginLeft: 'auto' }}
              onClick={handleSave}
              disabled={saving}
            >
              {saving ? 'Saving...' : 'Save Changes'}
            </button>
          </div>
        )}
      </div>

      {showWebhooks && (
        <WebhookManagerModal
          boardId={board.id}
          onClose={() => setShowWebhooks(false)}
          isMobile={isMobile}
        />
      )}
    </div>
  );
}

// ---- Activity Panel ----

const LAST_VISIT_KEY = (boardId) => `kanban_last_visit_${boardId}`;

function getLastVisit(boardId) {
  try { return localStorage.getItem(LAST_VISIT_KEY(boardId)); } catch { return null; }
}

function setLastVisit(boardId) {
  try { localStorage.setItem(LAST_VISIT_KEY(boardId), new Date().toISOString()); } catch {}
}

function formatEventDescription(event) {
  const { event_type, actor, data, task_title } = event;
  const who = actor || 'Someone';
  const title = task_title || '(untitled)';
  const truncTitle = title.length > 40 ? title.slice(0, 37) + '...' : title;

  switch (event_type) {
    case 'created': return `${who} created "${truncTitle}"`;
    case 'updated': return `${who} updated "${truncTitle}"`;
    case 'comment': {
      const msg = data?.message || '';
      const preview = msg.length > 60 ? msg.slice(0, 57) + '...' : msg;
      return `${who} commented on "${truncTitle}": ${preview}`;
    }
    case 'moved': {
      const to = data?.to_column || '';
      return `${who} moved "${truncTitle}"${to ? ` → ${to}` : ''}`;
    }
    case 'claimed': return `${who} claimed "${truncTitle}"`;
    case 'released': return `${who} released "${truncTitle}"`;
    case 'deleted': return `${who} deleted "${truncTitle}"`;
    case 'archived': return `${who} archived "${truncTitle}"`;
    case 'unarchived': return `${who} unarchived "${truncTitle}"`;
    default: return `${who} ${event_type} "${truncTitle}"`;
  }
}

function formatTimeAgo(dateStr) {
  const now = new Date();
  const d = parseUTC(dateStr);
  const diff = Math.floor((now - d) / 1000);
  if (diff < 60) return 'just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  return d.toLocaleDateString();
}

function eventIcon(type) {
  switch (type) {
    case 'created': return '✨';
    case 'updated': return '✏️';
    case 'comment': return '💬';
    case 'moved': return '➡️';
    case 'claimed': return '🙋';
    case 'released': return '🔓';
    case 'deleted': return '🗑️';
    case 'archived': return '📦';
    case 'unarchived': return '📤';
    default: return '📌';
  }
}

function ActivityPanel({ boardId, onClose, isMobile, onOpenTask }) {
  useEscapeKey(onClose);
  const lastVisitInit = getLastVisit(boardId);
  const [tab, setTab] = useState('mine'); // 'mine' | 'myactivity' | 'all'
  const [activity, setActivity] = useState([]);
  const [myTasks, setMyTasks] = useState([]);
  const [myActivity, setMyActivity] = useState([]);
  const [loading, setLoading] = useState(true);
  const [sortMode, setSortMode] = useState('priority'); // 'priority' | 'column'
  const lastVisit = getLastVisit(boardId);
  const displayName = api.getDisplayName();

  // Load recent activity
  useEffect(() => {
    if (tab !== 'all') return;
    setLoading(true);
    (async () => {
      try {
        const { data } = await api.getBoardActivity(boardId, { limit: 50 });
        setActivity(data || []);
      } catch (err) {
        console.error('Failed to load activity:', err);
      } finally {
        setLoading(false);
      }
    })();
  }, [boardId, tab]);

  // Load my items (assigned tasks + activity where I'm actor)
  useEffect(() => {
    if (tab !== 'mine' && tab !== 'myactivity') return;
    if (!displayName) { setLoading(false); return; }
    setLoading(true);
    (async () => {
      try {
        const [tasksRes, activityRes] = await Promise.all([
          api.listTasks(boardId, `assigned=${encodeURIComponent(displayName)}`),
          api.getBoardActivity(boardId, { limit: 200 }),
        ]);
        setMyTasks((tasksRes.data || []).filter(t => !t.archived_at));
        // Filter activity to events where the current user is mentioned or is the actor
        const dn = displayName.toLowerCase();
        const relevant = (activityRes.data || []).filter(e =>
          (e.actor && e.actor.toLowerCase() === dn) ||
          (e.data?.assigned_to && e.data.assigned_to.toLowerCase() === dn) ||
          (e.mentions && e.mentions.some(m => m.toLowerCase() === dn)) ||
          (e.data?.message && e.data.message.toLowerCase().includes('@' + dn))
        );
        setMyActivity(relevant.slice(0, 50));
      } catch (err) {
        console.error('Failed to load my items:', err);
      } finally {
        setLoading(false);
      }
    })();
  }, [boardId, tab, displayName]);

  const handleClose = () => {
    setLastVisit(boardId);
    onClose();
  };

  // Group tasks by column
  const tasksByColumn = {};
  myTasks.forEach(t => {
    const col = t.column_name || 'Unknown';
    if (!tasksByColumn[col]) tasksByColumn[col] = [];
    tasksByColumn[col].push(t);
  });

  // Sort tasks by priority (lower number = higher importance)
  const tasksByPriority = [...myTasks].sort((a, b) => (a.priority ?? 3) - (b.priority ?? 3));

  const tabStyle = (active) => ({
    background: active ? '#6366f133' : 'transparent',
    color: active ? '#a5b4fc' : '#94a3b8',
    border: `1px solid ${active ? '#6366f155' : '#334155'}`,
    borderRadius: '6px 6px 0 0',
    borderBottom: active ? '1px solid transparent' : '1px solid #334155',
    padding: '6px 14px',
    fontSize: '0.8rem',
    cursor: 'pointer',
    fontWeight: active ? '600' : '400',
    height: '32px',
    display: 'inline-flex',
    alignItems: 'center',
    gap: '6px',
  });

  const priorityColor = (p) => {
    if (p === 0) return '#ef4444';
    if (p === 1) return '#f59e0b';
    if (p === 2) return '#3b82f6';
    return '#64748b';
  };

  const renderActivityList = () => {
    return (
      <>
        {activity.length === 0 ? (
          <div style={{ color: '#64748b', textAlign: 'center', padding: '20px' }}>
            No activity yet.
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '2px', overflow: 'auto', maxHeight: isMobile ? 'calc(100vh - 200px)' : '55vh' }}>
            {activity.map(event => (
              <div key={event.id} style={{
                padding: '8px 10px',
                borderRadius: '4px',
                background: '#1e293b',
                border: '1px solid #1e293b',
                fontSize: '0.8rem',
                lineHeight: '1.4',
              }}>
                <div style={{ display: 'flex', gap: '6px', alignItems: 'flex-start' }}>
                  <span style={{ flexShrink: 0 }}>{eventIcon(event.event_type)}</span>
                  <span style={{ color: '#e2e8f0', flex: 1 }}>
                    {formatEventDescription(event)}
                  </span>
                  <span style={{ color: '#64748b', fontSize: '0.7rem', flexShrink: 0, whiteSpace: 'nowrap' }}>
                    {formatTimeAgo(event.created_at)}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}
      </>
    );
  };

  const sortToggleStyle = (active) => ({
    background: active ? '#6366f133' : '#334155',
    color: active ? '#a5b4fc' : '#94a3b8',
    border: `1px solid ${active ? '#6366f155' : '#475569'}`,
    borderRadius: '4px',
    padding: '3px 10px',
    fontSize: '0.7rem',
    cursor: 'pointer',
    fontWeight: active ? '600' : '400',
  });

  const renderTaskItem = (task) => (
    <div
      key={task.id}
      onClick={() => { if (onOpenTask) onOpenTask(task); }}
      style={{
        padding: '8px 10px',
        borderRadius: '4px',
        background: '#1e293b',
        border: '1px solid #2a3548',
        fontSize: '0.8rem',
        lineHeight: '1.4',
        marginBottom: '2px',
        cursor: onOpenTask ? 'pointer' : 'default',
        display: 'flex',
        alignItems: 'center',
        gap: '8px',
      }}
    >
      <span style={{ color: priorityColor(task.priority), fontSize: '0.7rem', fontWeight: '700', flexShrink: 0 }}>
        P{task.priority}
      </span>
      <span style={{ color: task.title ? '#e2e8f0' : '#94a3b8', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', fontStyle: task.title ? 'normal' : 'italic' }}>
        {task.title || (task.description ? task.description.slice(0, 60) + (task.description.length > 60 ? '…' : '') : '(untitled)')}
      </span>
      {sortMode === 'priority' && task.column_name && (
        <span style={{ color: '#64748b', fontSize: '0.65rem', flexShrink: 0 }}>
          {task.column_name}
        </span>
      )}
      {task.comment_count > 0 && (
        <span style={{ color: '#64748b', fontSize: '0.7rem', flexShrink: 0 }}>
          💬{task.comment_count}
        </span>
      )}
    </div>
  );

  const renderMyItemsTab = () => {
    if (!displayName) {
      return (
        <div style={{ color: '#64748b', textAlign: 'center', padding: '20px' }}>
          Set a display name to see your assigned tasks.
        </div>
      );
    }

    return (
      <div style={{ overflow: 'auto', maxHeight: isMobile ? 'calc(100vh - 200px)' : '55vh' }}>
        {/* Header with count and sort toggle */}
        {myTasks.length > 0 && (
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
            <div style={{ color: '#94a3b8', fontSize: '0.7rem', fontWeight: '600', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
              Assigned to me ({myTasks.length})
            </div>
            <div style={{ display: 'flex', gap: '4px' }}>
              <button style={sortToggleStyle(sortMode === 'priority')} onClick={() => setSortMode('priority')}>
                By Priority
              </button>
              <button style={sortToggleStyle(sortMode === 'column')} onClick={() => setSortMode('column')}>
                By Column
              </button>
            </div>
          </div>
        )}

        {myTasks.length > 0 ? (
          <div style={{ marginBottom: '16px' }}>
            {sortMode === 'priority' ? (
              /* Priority sorted: flat list, highest priority (lowest number) first */
              tasksByPriority.map(task => renderTaskItem(task))
            ) : (
              /* Grouped by column */
              Object.entries(tasksByColumn).map(([colName, tasks]) => (
                <div key={colName} style={{ marginBottom: '10px' }}>
                  <div style={{ color: '#64748b', fontSize: '0.7rem', marginBottom: '4px', paddingLeft: '4px' }}>
                    {colName}
                  </div>
                  {tasks.map(task => renderTaskItem(task))}
                </div>
              ))
            )}
          </div>
        ) : (
          <div style={{ color: '#64748b', textAlign: 'center', padding: '16px', fontSize: '0.8rem' }}>
            No tasks assigned to you.
          </div>
        )}
      </div>
    );
  };

  const renderMyRecentActivityTab = () => {
    if (!displayName) {
      return (
        <div style={{ color: '#64748b', textAlign: 'center', padding: '20px' }}>
          Set a display name to see your recent activity.
        </div>
      );
    }

    return (
      <div style={{ overflow: 'auto', maxHeight: isMobile ? 'calc(100vh - 200px)' : '55vh' }}>
        {myActivity.length > 0 ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '2px' }}>
            {myActivity.map(event => (
              <div key={event.id} style={{
                padding: '8px 10px',
                borderRadius: '4px',
                background: '#1e293b',
                border: '1px solid #1e293b',
                fontSize: '0.8rem',
                lineHeight: '1.4',
              }}>
                <div style={{ display: 'flex', gap: '6px', alignItems: 'flex-start' }}>
                  <span style={{ flexShrink: 0 }}>{eventIcon(event.event_type)}</span>
                  <span style={{ color: '#e2e8f0', flex: 1 }}>
                    {formatEventDescription(event)}
                  </span>
                  <span style={{ color: '#64748b', fontSize: '0.7rem', flexShrink: 0, whiteSpace: 'nowrap' }}>
                    {formatTimeAgo(event.created_at)}
                  </span>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div style={{ color: '#64748b', textAlign: 'center', padding: '20px' }}>
            No recent activity.
          </div>
        )}
      </div>
    );
  };

  return (
    <div style={styles.modal(isMobile)} onClick={handleClose}>
      <div style={{ ...styles.modalContent(isMobile), width: isMobile ? '100%' : '560px', maxHeight: isMobile ? '100vh' : '85vh' }} onClick={e => e.stopPropagation()}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
          <h2 style={{ color: '#f1f5f9', fontSize: '1.1rem', margin: 0, display: 'flex', alignItems: 'center', gap: '8px' }}><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg> Activity</h2>
          <button style={styles.btnClose} onClick={handleClose}>×</button>
        </div>

        {/* Tab bar */}
        <div style={{ display: 'flex', gap: '4px', marginBottom: '12px', borderBottom: '1px solid #334155' }}>
          <button style={tabStyle(tab === 'mine')} onClick={() => setTab('mine')}>
            👤 My Items
            {myTasks.length > 0 && (
              <span style={{
                background: '#f59e0b',
                color: '#1e293b',
                borderRadius: '8px',
                padding: '1px 6px',
                fontSize: '0.65rem',
                fontWeight: '700',
              }}>{myTasks.length}</span>
            )}
          </button>
          <button style={tabStyle(tab === 'myactivity')} onClick={() => setTab('myactivity')}>
            🕐 My Recent Activity
          </button>
          <button style={tabStyle(tab === 'all')} onClick={() => setTab('all')}>
            📋 All Recent
          </button>
        </div>

        {loading ? (
          <div style={{ color: '#64748b', textAlign: 'center', padding: '20px' }}>Loading...</div>
        ) : tab === 'mine' ? renderMyItemsTab() : tab === 'myactivity' ? renderMyRecentActivityTab() : renderActivityList()}
      </div>
    </div>
  );
}

function WebhookManagerModal({ boardId, onClose, isMobile }) {
  useEscapeKey(onClose);
  const [webhooks, setWebhooks] = useState([]);
  const [loading, setLoading] = useState(true);
  const [showAdd, setShowAdd] = useState(false);
  const [newUrl, setNewUrl] = useState('');
  const [newEvents, setNewEvents] = useState([]);
  const [createdSecret, setCreatedSecret] = useState(null);
  const [error, setError] = useState('');

  const loadWebhooks = useCallback(async () => {
    try {
      const { data } = await api.listWebhooks(boardId);
      setWebhooks(data || []);
    } catch (err) {
      setError(err.error || 'Failed to load webhooks');
    } finally {
      setLoading(false);
    }
  }, [boardId]);

  useEffect(() => { loadWebhooks(); }, [loadWebhooks]);

  const handleCreate = async () => {
    setError('');
    if (!newUrl.trim()) { setError('URL is required'); return; }
    try {
      const { data } = await api.createWebhook(boardId, {
        url: newUrl.trim(),
        events: newEvents.length > 0 ? newEvents : [],
      });
      setCreatedSecret(data.secret);
      setNewUrl('');
      setNewEvents([]);
      setShowAdd(false);
      loadWebhooks();
    } catch (err) {
      setError(err.error || 'Failed to create webhook');
    }
  };

  const handleToggle = async (wh) => {
    try {
      await api.updateWebhook(boardId, wh.id, { active: !wh.active });
      loadWebhooks();
    } catch (err) {
      setError(err.error || 'Failed to update webhook');
    }
  };

  const handleDelete = async (wh) => {
    if (!confirm(`Delete webhook to ${wh.url}?`)) return;
    try {
      await api.deleteWebhook(boardId, wh.id);
      loadWebhooks();
    } catch (err) {
      setError(err.error || 'Failed to delete webhook');
    }
  };

  const toggleEvent = (evt) => {
    setNewEvents(prev =>
      prev.includes(evt) ? prev.filter(e => e !== evt) : [...prev, evt]
    );
  };

  return (
    <div style={styles.modal(isMobile)} onClick={onClose}>
      <div style={styles.modalContentWide(isMobile)} onClick={e => e.stopPropagation()}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
          <h2 style={{ color: '#f1f5f9', fontSize: '1.1rem', margin: 0 }}>⚡ Webhooks</h2>
          <button style={styles.btnClose} onClick={onClose}>×</button>
        </div>

        {error && (
          <div style={{ background: '#ef444422', border: '1px solid #ef444444', borderRadius: '4px', padding: '8px 12px', marginBottom: '12px', color: '#fca5a5', fontSize: '0.8rem' }}>
            {error}
          </div>
        )}

        {createdSecret && (
          <div style={styles.successBox}>
            <div style={{ color: '#22c55e', fontWeight: 600, fontSize: '0.85rem', marginBottom: '6px' }}>✅ Webhook created!</div>
            <div style={{ fontSize: '0.78rem', color: '#94a3b8', marginBottom: '6px' }}>
              Save this secret — it's shown only once. Use it to verify webhook signatures.
            </div>
            <div style={styles.urlBox}>
              <span style={{ flex: 1, color: '#e2e8f0' }}>{createdSecret}</span>
              <button style={styles.btnSmall} onClick={() => { navigator.clipboard.writeText(createdSecret); }}>Copy</button>
            </div>
            <button style={{ ...styles.btnSmall, marginTop: '6px' }} onClick={() => setCreatedSecret(null)}>Close</button>
          </div>
        )}

        {loading ? (
          <div style={{ color: '#64748b', fontSize: '0.85rem', padding: '20px 0', textAlign: 'center' }}>Loading…</div>
        ) : (
          <>
            {webhooks.length === 0 && !showAdd && (
              <div style={{ color: '#64748b', fontSize: '0.85rem', padding: '20px 0', textAlign: 'center' }}>
                No webhooks configured. Webhooks notify external services when tasks change.
              </div>
            )}

            {webhooks.map(wh => (
              <div key={wh.id} style={{
                background: '#0f172a', border: '1px solid #334155', borderRadius: '6px',
                padding: '12px', marginBottom: '8px',
              }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: '8px' }}>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontFamily: 'monospace', fontSize: '0.78rem', color: '#e2e8f0', wordBreak: 'break-all' }}>
                      {wh.url}
                    </div>
                    <div style={{ fontSize: '0.7rem', color: '#64748b', marginTop: '4px' }}>
                      {wh.events.length === 0 ? 'All events' : wh.events.join(', ')}
                      {wh.failure_count > 0 && (
                        <span style={{ color: '#ef4444', marginLeft: '8px' }}>⚠️ {wh.failure_count} failures</span>
                      )}
                    </div>
                  </div>
                  <div style={{ display: 'flex', gap: '6px', alignItems: 'center', flexShrink: 0 }}>
                    <button
                      style={{
                        ...styles.btnSmall,
                        background: wh.active ? '#22c55e22' : '#ef444422',
                        borderColor: wh.active ? '#22c55e44' : '#ef444444',
                        color: wh.active ? '#22c55e' : '#ef4444',
                      }}
                      onClick={() => handleToggle(wh)}
                    >
                      {wh.active ? 'Active' : 'Paused'}
                    </button>
                    <button
                      style={{ ...styles.btnSmall, color: '#ef4444', borderColor: '#ef444444' }}
                      onClick={() => handleDelete(wh)}
                    >🗑️</button>
                  </div>
                </div>
              </div>
            ))}

            {showAdd ? (
              <div style={{ background: '#0f172a', border: '1px solid #334155', borderRadius: '6px', padding: '12px', marginTop: '8px' }}>
                <div style={{ fontSize: '0.8rem', color: '#94a3b8', marginBottom: '8px', fontWeight: 600 }}>New Webhook</div>
                <input
                  autoFocus
                  style={styles.input}
                  placeholder="https://example.com/webhook"
                  value={newUrl}
                  onChange={e => setNewUrl(e.target.value)}
                  onKeyDown={e => { if (e.key === 'Enter') handleCreate(); if (e.key === 'Escape') setShowAdd(false); }}
                />
                <div style={{ fontSize: '0.75rem', color: '#64748b', marginBottom: '6px' }}>
                  Events (leave empty for all):
                </div>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px', marginBottom: '12px' }}>
                  {WEBHOOK_EVENTS.map(evt => (
                    <button
                      key={evt}
                      onClick={() => toggleEvent(evt)}
                      style={{
                        ...styles.btnSmall,
                        background: newEvents.includes(evt) ? '#6366f133' : 'transparent',
                        borderColor: newEvents.includes(evt) ? '#6366f1' : '#334155',
                        color: newEvents.includes(evt) ? '#a5b4fc' : '#64748b',
                        fontSize: '0.7rem',
                      }}
                    >{evt}</button>
                  ))}
                </div>
                <div style={{ display: 'flex', gap: '8px' }}>
                  <button style={styles.btn('primary', isMobile)} onClick={handleCreate}>Create</button>
                  <button style={styles.btn('secondary', isMobile)} onClick={() => { setShowAdd(false); setNewUrl(''); setNewEvents([]); }}>Cancel</button>
                </div>
              </div>
            ) : (
              <button
                style={{ ...styles.btn('primary', isMobile), marginTop: '8px' }}
                onClick={() => setShowAdd(true)}
              >+ Add Webhook</button>
            )}
          </>
        )}
      </div>
    </div>
  );
}

// ---- Live SSE Connection Indicator (Header) ----
// Desktop: pill tag with "LIVE" text to the left of the username.
// Mobile: small dot only.
function LiveIndicator({ status, isMobile }) {
  if (!status || status === 'initial') return null;
  const connected = status === 'connected';
  const color = connected ? '#22c55e' : '#ef4444';
  const title = connected ? 'Live — real-time sync active' : 'Reconnecting…';

  // Mobile: compact dot only
  if (isMobile) {
    return (
      <span title={title} style={{
        display: 'inline-flex', alignItems: 'center', gap: '5px',
        cursor: 'default',
      }}>
        <span style={{
          width: '7px', height: '7px',
          borderRadius: '50%', background: color, flexShrink: 0,
          boxShadow: connected ? `0 0 5px ${color}80` : 'none',
          animation: connected ? 'ssePulse 2.5s ease-in-out infinite' : 'none',
        }} />
        {!connected && (
          <span style={{ fontSize: '0.6rem', color: '#fca5a5', fontWeight: 500, whiteSpace: 'nowrap' }}>
            Reconnecting…
          </span>
        )}
      </span>
    );
  }

  // Desktop: pill tag with "LIVE" text
  return (
    <span title={title} style={{
      display: 'inline-flex', alignItems: 'center', gap: '5px',
      cursor: 'default',
      background: connected ? '#22c55e18' : '#ef444418',
      border: `1px solid ${connected ? '#22c55e40' : '#ef444440'}`,
      borderRadius: '9999px',
      padding: '2px 8px 2px 6px',
      fontSize: '0.65rem',
      fontWeight: 600,
      letterSpacing: '0.04em',
      color: connected ? '#4ade80' : '#fca5a5',
      textTransform: 'uppercase',
      whiteSpace: 'nowrap',
    }}>
      <span style={{
        width: '6px', height: '6px',
        borderRadius: '50%', background: color, flexShrink: 0,
        boxShadow: connected ? `0 0 4px ${color}80` : 'none',
        animation: connected ? 'ssePulse 2.5s ease-in-out infinite' : 'none',
      }} />
      {connected ? 'Live' : 'Reconnecting…'}
    </span>
  );
}

// ---- Share / Access Popover ----
function SharePopover({ boardId, canEdit, onClose }) {
  const origin = window.location.origin;
  const viewUrl = `${origin}/board/${boardId}`;
  const manageKey = api.getBoardKey(boardId);
  const manageUrl = manageKey ? `${viewUrl}?key=${manageKey}` : null;
  const [copied, setCopied] = useState(null);

  const copy = (text, label) => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(label);
      setTimeout(() => setCopied(null), 1500);
    });
  };

  const mobile = window.innerWidth < 640;

  return (
    <>
      <div style={{ position: 'fixed', inset: 0, zIndex: 299, background: mobile ? 'rgba(0,0,0,0.6)' : 'transparent' }} onClick={onClose} />
      <div style={mobile ? {
        position: 'fixed', inset: 0, zIndex: 300,
        background: '#1e293b', padding: '16px',
        display: 'flex', flexDirection: 'column',
        overflow: 'auto',
      } : {
        position: 'absolute', top: '100%', right: 0, marginTop: '6px',
        zIndex: 300, background: '#1e293b', border: '1px solid #334155', borderRadius: '8px',
        padding: '16px', width: '320px', maxWidth: '90vw',
        boxShadow: '0 8px 24px rgba(0,0,0,0.4)',
      }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
          <div style={{ fontSize: mobile ? '1rem' : '0.75rem', fontWeight: 600, color: '#94a3b8', textTransform: 'uppercase', letterSpacing: '0.04em' }}>
            Share Board
          </div>
          <button onClick={onClose} style={styles.btnClose}>×</button>
        </div>

        {/* View URL */}
        <div style={{ marginBottom: canEdit ? '10px' : 0 }}>
          <div style={{ fontSize: mobile ? '0.85rem' : '0.7rem', color: '#64748b', marginBottom: '4px' }}>👁️ Read-only link — anyone with this can view</div>
          <div style={{ display: 'flex', gap: '6px', alignItems: 'center' }}>
            <input readOnly value={viewUrl} style={{
              flex: 1, background: '#0f172a', color: '#e2e8f0', border: '1px solid #334155',
              borderRadius: '4px', padding: mobile ? '10px' : '5px 8px', fontSize: mobile ? '16px' : '0.75rem', outline: 'none',
            }} onClick={e => e.target.select()} />
            <button onClick={() => copy(viewUrl, 'view')} style={{
              background: copied === 'view' ? '#22c55e22' : '#334155', color: copied === 'view' ? '#22c55e' : '#e2e8f0',
              border: 'none', borderRadius: '4px', padding: mobile ? '10px 14px' : '5px 8px', cursor: 'pointer', fontSize: mobile ? '16px' : '0.75rem', whiteSpace: 'nowrap',
            }}>{copied === 'view' ? '✓ Copied' : 'Copy'}</button>
          </div>
        </div>

        {/* Manage URL */}
        {canEdit && manageUrl && (
          <div>
            <div style={{ fontSize: mobile ? '0.85rem' : '0.7rem', color: '#64748b', marginBottom: '4px' }}>✏️ Edit link — full access (keep private!)</div>
            <div style={{ display: 'flex', gap: '6px', alignItems: 'center' }}>
              <input readOnly value={manageUrl} style={{
                flex: 1, background: '#0f172a', color: '#e2e8f0', border: '1px solid #334155',
                borderRadius: '4px', padding: mobile ? '10px' : '5px 8px', fontSize: mobile ? '16px' : '0.75rem', outline: 'none',
              }} onClick={e => e.target.select()} />
              <button onClick={() => copy(manageUrl, 'manage')} style={{
                background: copied === 'manage' ? '#22c55e22' : '#334155', color: copied === 'manage' ? '#22c55e' : '#e2e8f0',
                border: 'none', borderRadius: '4px', padding: mobile ? '10px 14px' : '5px 8px', cursor: 'pointer', fontSize: mobile ? '16px' : '0.75rem', whiteSpace: 'nowrap',
              }}>{copied === 'manage' ? '✓ Copied' : 'Copy'}</button>
            </div>
          </div>
        )}

        {/* Hint for view-only users */}
        {!canEdit && (
          <div style={{ fontSize: mobile ? '0.85rem' : '0.7rem', color: '#475569', marginTop: '8px', lineHeight: 1.4 }}>
            Need edit access? Open the board using the manage link (contains <code style={{ color: '#94a3b8' }}>?key=...</code>).
          </div>
        )}
      </div>
    </>
  );
}

// ---- Access Mode Indicator + Share ----
function AccessIndicator({ boardId, canEdit, isMobile, onKeyUpgraded }) {
  const [showShare, setShowShare] = useState(false);
  const [showModeInfo, setShowModeInfo] = useState(false);
  const [keyInput, setKeyInput] = useState('');
  const [keyError, setKeyError] = useState('');
  const [validating, setValidating] = useState(false);

  const handleUnlock = async () => {
    const key = keyInput.trim();
    if (!key) return;
    setKeyError('');
    setValidating(true);
    try {
      const valid = await api.validateKey(boardId, key);
      if (valid) {
        api.setBoardKey(boardId, key);
        setShowModeInfo(false);
        setKeyInput('');
        if (onKeyUpgraded) onKeyUpgraded();
      } else {
        setKeyError('Invalid key — please check and try again.');
      }
    } catch {
      setKeyError('Could not validate key. Try again.');
    }
    setValidating(false);
  };

  return (
    <div style={{ position: 'relative', display: 'inline-flex', alignItems: 'center', gap: 0 }}>
      <button
        onClick={() => { setShowModeInfo(v => !v); setKeyError(''); setKeyInput(''); }}
        style={{
          fontSize: '0.7rem', fontWeight: 600,
          padding: '3px 8px', borderRadius: '12px 0 0 12px',
          background: canEdit ? '#22c55e15' : '#64748b15',
          color: canEdit ? '#22c55e' : '#94a3b8',
          border: `1px solid ${canEdit ? '#22c55e33' : '#64748b33'}`,
          borderRight: 'none', whiteSpace: 'nowrap',
          cursor: 'pointer',
        }}
        title={canEdit ? 'Full access mode' : 'Click to enter manage key'}
      >
        {canEdit ? (isMobile ? '✏️' : '✏️ Full Access') : (isMobile ? '👁️' : '👁️ View Only')}
      </button>
      {showModeInfo && (
        <>
        <div style={{ position: 'fixed', inset: 0, zIndex: 1999, background: isMobile ? 'rgba(0,0,0,0.6)' : 'transparent' }} onClick={() => setShowModeInfo(false)} />
        <div
          onClick={e => e.stopPropagation()}
          style={isMobile ? {
            position: 'fixed', inset: 0, zIndex: 2000,
            background: '#1e293b', padding: '16px',
            display: 'flex', flexDirection: 'column',
            overflow: 'auto',
            fontSize: '0.9rem', color: '#cbd5e1', lineHeight: '1.5',
          } : {
            position: 'absolute', top: '100%', right: 0, marginTop: '6px',
            background: '#1e293b', border: '1px solid #334155', borderRadius: '8px',
            padding: '12px', width: '320px', zIndex: 2000,
            boxShadow: '0 8px 24px rgba(0,0,0,0.5)',
            fontSize: '0.78rem', color: '#cbd5e1', lineHeight: '1.5',
          }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
            <div style={{ fontWeight: 700, color: '#f1f5f9', fontSize: isMobile ? '1.1rem' : 'inherit' }}>
              {canEdit ? '✏️ Full Access Mode' : '👁️ View Only Mode'}
            </div>
            <button onClick={() => setShowModeInfo(false)} style={styles.btnClose}>×</button>
          </div>
          {canEdit ? (
            <div>
              <p style={{ margin: '0 0 6px' }}>You have the <strong style={{ color: '#22c55e' }}>manage key</strong> for this board. You can:</p>
              <ul style={{ margin: '0 0 6px', paddingLeft: '16px' }}>
                <li>Create, edit, and delete tasks</li>
                <li>Add and manage columns</li>
                <li>Post comments</li>
                <li>Archive tasks and the board</li>
                <li>Change board settings</li>
              </ul>
              <p style={{ margin: 0, fontSize: isMobile ? '0.85rem' : '0.72rem', color: '#94a3b8' }}>Share the <strong>View URL</strong> for read-only access, or the <strong>Manage URL</strong> to grant full access.</p>
            </div>
          ) : (
            <div>
              <p style={{ margin: '0 0 6px' }}>You're viewing this board in <strong style={{ color: '#94a3b8' }}>read-only</strong> mode.</p>
              <div style={{
                marginTop: '10px', padding: isMobile ? '14px' : '10px', background: '#0f172a',
                borderRadius: '6px', border: '1px solid #334155',
              }}>
                <div style={{ fontWeight: 600, color: '#f1f5f9', marginBottom: '6px', fontSize: isMobile ? '0.9rem' : '0.75rem' }}>
                  🔑 Have a manage key?
                </div>
                <div style={{ display: 'flex', gap: '6px' }}>
                  <input
                    type="text"
                    value={keyInput}
                    onChange={e => { setKeyInput(e.target.value); setKeyError(''); }}
                    onKeyDown={e => { if (e.key === 'Enter') handleUnlock(); }}
                    placeholder="Paste manage key..."
                    style={{
                      flex: 1, padding: isMobile ? '10px' : '5px 8px', fontSize: '16px',
                      background: '#1e293b', color: '#f1f5f9',
                      border: `1px solid ${keyError ? '#ef4444' : '#475569'}`,
                      borderRadius: '4px', outline: 'none',
                    }}
                    disabled={validating}
                  />
                  <button
                    onClick={handleUnlock}
                    disabled={validating || !keyInput.trim()}
                    style={{
                      padding: isMobile ? '10px 14px' : '5px 10px', fontSize: isMobile ? '16px' : '0.72rem', fontWeight: 600,
                      background: validating ? '#475569' : '#3b82f6',
                      color: '#fff', border: 'none', borderRadius: '4px',
                      cursor: validating ? 'wait' : 'pointer',
                      opacity: !keyInput.trim() ? 0.5 : 1,
                    }}
                  >
                    {validating ? '...' : 'Unlock'}
                  </button>
                </div>
                {keyError && (
                  <div style={{ color: '#ef4444', fontSize: isMobile ? '0.85rem' : '0.7rem', marginTop: '4px' }}>{keyError}</div>
                )}
                <p style={{ margin: '6px 0 0', fontSize: isMobile ? '0.8rem' : '0.68rem', color: '#64748b' }}>
                  Or open the <strong>Manage URL</strong> (contains <code style={{ background: '#1e293b', padding: '1px 3px', borderRadius: '2px' }}>?key=</code>) from the board owner.
                </p>
              </div>
            </div>
          )}
        </div>
        </>
      )}
      <button
        onClick={() => setShowShare(s => !s)}
        style={{
          fontSize: '0.7rem', fontWeight: 600,
          padding: '3px 8px', borderRadius: '0 12px 12px 0',
          background: showShare ? '#3b82f622' : (canEdit ? '#22c55e15' : '#64748b15'),
          color: showShare ? '#3b82f6' : (canEdit ? '#22c55e' : '#94a3b8'),
          border: `1px solid ${canEdit ? '#22c55e33' : '#64748b33'}`,
          cursor: 'pointer', whiteSpace: 'nowrap',
        }}
        title="Share board"
      >
        {isMobile ? '🔗' : '🔗 Share'}
      </button>
      {showShare && <SharePopover boardId={boardId} canEdit={canEdit} onClose={() => setShowShare(false)} />}
    </div>
  );
}

function BoardView({ board, canEdit, onRefresh, onBoardRefresh, onBoardListRefresh, isMobile, onSseStatusChange }) {
  const [tasks, setTasks] = useState([]);
  const [showCreate, setShowCreate] = useState(false);
  const [search, setSearch] = useState('');
  const [searchResults, setSearchResults] = useState(null);
  const searchRef = useRef({ search: '', hasResults: false });
  const [selectedTask, setSelectedTask] = useState(null);
  const [sseStatus, setSseStatus] = useState('initial');
  const [addingColumn, setAddingColumn] = useState(false);
  const [newColumnName, setNewColumnName] = useState('');
  // showWebhooks state removed — webhook button removed from UI per Jordan's request
  const [showSettings, setShowSettings] = useState(false);
  const [showActivity, setShowActivity] = useState(false);
  const [filterPriority, setFilterPriority] = useState('');
  const [filterLabel, setFilterLabel] = useState('');
  const [filterAssignee, setFilterAssignee] = useState('');
  const [filterCreatedBy, setFilterCreatedBy] = useState('');
  const [showFilters, setShowFilters] = useState(false);
  const [showArchivedTasks, setShowArchivedTasks] = useState(false);
  const [showSearchBar, setShowSearchBar] = useState(!isMobile);
  const [collapsedColumns, setCollapsedColumns] = useState({});
  const [tasksLoaded, setTasksLoaded] = useState(false);
  const [newActivityCount, setNewActivityCount] = useState(0);
  const [fullScreenColumnId, setFullScreenColumnId] = useState(null);
  const toggleColumnCollapse = useCallback((colId) => {
    setCollapsedColumns(prev => ({ ...prev, [colId]: !prev[colId] }));
  }, []);

  const loadTasks = useCallback(async () => {
    try {
      const params = showArchivedTasks ? 'archived=true' : '';
      const { data } = await api.listTasks(board.id, params);
      setTasks(data.tasks || data || []);
      setTasksLoaded(true);
    } catch (err) {
      console.error('Failed to load tasks:', err);
    }
  }, [board.id, showArchivedTasks]);

  useEffect(() => { loadTasks(); }, [loadTasks]);

  // Sync selectedTask with refreshed tasks data (fixes stale view after edit/save)
  useEffect(() => {
    if (selectedTask) {
      const updated = tasks.find(t => t.id === selectedTask.id);
      if (updated && JSON.stringify(updated) !== JSON.stringify(selectedTask)) {
        setSelectedTask(updated);
      } else if (!updated && !showArchivedTasks) {
        // Task may have been deleted or archived
        setSelectedTask(null);
      }
    }
  }, [tasks]);

  // Load new activity count for the badge
  useEffect(() => {
    const lv = getLastVisit(board.id);
    if (!lv) { setNewActivityCount(0); return; }
    (async () => {
      try {
        const { data } = await api.getBoardActivity(board.id, { since: lv, limit: 100 });
        setNewActivityCount((data || []).length);
      } catch { setNewActivityCount(0); }
    })();
  }, [board.id, showActivity]);

  // SSE: subscribe to real-time board events (debounced refresh)
  useEffect(() => {
    let debounceTimer = null;
    const debouncedRefresh = () => {
      if (debounceTimer) clearTimeout(debounceTimer);
      debounceTimer = setTimeout(async () => {
        await loadTasks();
        // Also refresh search results if a search is active
        const { search: q, hasResults } = searchRef.current;
        if (hasResults && q.trim()) {
          try {
            const { data } = await api.searchTasks(board.id, q.trim());
            setSearchResults(data.tasks || []);
          } catch (err) { /* ignore */ }
        }
      }, 300);
    };
    const sub = api.subscribeToBoardEvents(
      board.id,
      (evt) => {
        // On any task event, debounce-refresh the task list
        if (evt.event !== 'warning') {
          debouncedRefresh();
        }
      },
      (status) => { setSseStatus(status); onSseStatusChange?.(status); }, // Feed status to header indicator
    );
    return () => {
      if (debounceTimer) clearTimeout(debounceTimer);
      sub.close();
    };
  }, [board.id, loadTasks]);

  // Keep ref in sync so SSE handler can check without being a dependency
  useEffect(() => {
    searchRef.current = { search, hasResults: searchResults !== null };
  }, [search, searchResults]);

  const doSearch = async () => {
    if (!search.trim()) { setSearchResults(null); return; }
    try {
      const { data } = await api.searchTasks(board.id, search.trim());
      setSearchResults(data.tasks || []);
    } catch (err) {
      console.error('Search failed:', err);
    }
  };

  // Refresh both tasks and search results (if active) so the board stays in sync
  const refreshAll = useCallback(async () => {
    await loadTasks();
    if (searchResults !== null && search.trim()) {
      try {
        const { data } = await api.searchTasks(board.id, search.trim());
        setSearchResults(data.tasks || []);
      } catch (err) {
        console.error('Search refresh failed:', err);
      }
    }
  }, [loadTasks, searchResults, search, board.id]);

  const columns = board.columns || [];
  const baseTasks = searchResults !== null ? searchResults : tasks;

  // Collect unique labels and assignees for filter dropdowns
  const allLabels = (() => {
    const counts = {};
    baseTasks.forEach(t => {
      (Array.isArray(t.labels) ? t.labels : (t.labels || '').split(',').map(l => l.trim())).filter(Boolean).forEach(l => {
        counts[l] = (counts[l] || 0) + 1;
      });
    });
    return Object.keys(counts).sort((a, b) => counts[b] - counts[a]);
  })();
  const allLabelsSorted = [...allLabels].sort((a, b) => a.localeCompare(b));
  const allAssignees = [...new Set(baseTasks.map(t => t.assigned_to || t.claimed_by).filter(Boolean))].sort();
  const allCreators = [...new Set(baseTasks.map(t => t.created_by).filter(Boolean))].sort();

  // Apply filters
  const displayTasks = baseTasks.filter(t => {
    if (filterPriority) {
      if (filterPriority === '3') { if ((t.priority || 0) < 3) return false; }
      else if (String(t.priority) !== filterPriority) return false;
    }
    if (filterLabel && !(Array.isArray(t.labels) ? t.labels : (t.labels || '').split(',').map(l => l.trim())).some(l => l.toLowerCase() === filterLabel.toLowerCase())) return false;
    if (filterAssignee && t.assigned_to !== filterAssignee && t.claimed_by !== filterAssignee) return false;
    if (filterCreatedBy && t.created_by !== filterCreatedBy) return false;
    return true;
  });
  const hasActiveFilters = filterPriority || filterLabel || filterAssignee || filterCreatedBy || showArchivedTasks;
  const searchActive = searchResults !== null || hasActiveFilters;
  const archived = !!board.archived_at;

  return (
    <div style={styles.boardContent(isMobile)}>
      <div style={styles.boardHeader(isMobile)}>
        <div style={{ minWidth: 0 }}>
          <span style={styles.boardTitle(isMobile)}>{board.name}</span>
          {archived && <span style={{ ...styles.archivedBadge, marginLeft: '10px' }}>ARCHIVED</span>}
          {board.description && (
            <p style={{ fontSize: '0.8rem', color: '#64748b', marginTop: '4px' }}>{board.description}</p>
          )}
        </div>
        {isMobile ? (
          /* Mobile: connected segmented button bar, 100% width */
          <div style={{ display: 'flex', width: '100%', borderRadius: '6px', overflow: 'hidden', border: '1px solid #475569' }}>
            <button style={{ flex: '1 1 0', background: '#334155', color: '#cbd5e1', border: 'none', borderRight: '1px solid #475569', padding: '10px 0', cursor: 'pointer', fontSize: '0.85rem', display: 'inline-flex', alignItems: 'center', justifyContent: 'center', position: 'relative' }} onClick={() => setShowActivity(true)} title="Activity Feed">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
              {newActivityCount > 0 && (
                <span style={{
                  position: 'absolute', top: '4px', right: '4px',
                  background: '#6366f1',
                  width: '8px', height: '8px',
                  borderRadius: '50%',
                }} />
              )}
            </button>
            <button style={{ flex: '1 1 0', background: '#334155', color: '#cbd5e1', border: 'none', borderRight: '1px solid #475569', padding: '10px 0', cursor: 'pointer', fontSize: '0.85rem', display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }} onClick={() => setShowSettings(true)} title="Board Settings">⚙️</button>
            <button style={{ flex: '1 1 0', background: searchActive ? '#312e81' : '#334155', color: searchActive ? '#a5b4fc' : '#cbd5e1', border: 'none', borderRight: '1px solid #475569', padding: '10px 0', cursor: 'pointer', fontSize: '0.85rem', display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }} onClick={() => setShowSearchBar(v => !v)} title="Search & Filter">🔍</button>
            {canEdit && !archived && (
              <button style={{ flex: '0 0 33.33%', background: '#6366f1', color: '#fff', border: 'none', padding: '10px 14px', cursor: 'pointer', fontSize: '0.9rem', fontWeight: 600, display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: '6px' }} onClick={() => setShowCreate(true)}>+ Task</button>
            )}
          </div>
        ) : (
          /* Desktop: original button layout */
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexShrink: 0, flexWrap: 'wrap' }}>
            <button style={{ ...styles.btn('secondary', false), position: 'relative' }} onClick={() => setShowActivity(true)} title="Activity Feed">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
              {newActivityCount > 0 && (
                <span style={{
                  position: 'absolute', top: '-2px', right: '-2px',
                  background: '#6366f1',
                  width: '8px', height: '8px',
                  borderRadius: '50%',
                }} />
              )}
            </button>
            <button style={styles.btn('secondary', false)} onClick={() => setShowSettings(true)} title="Board Settings">⚙️</button>
            {canEdit && !archived && (
              <button style={styles.btn('primary', false)} onClick={() => setShowCreate(true)}>+ Task</button>
            )}
          </div>
        )}
      </div>

      {showSearchBar && (
        <div style={styles.searchBar(isMobile)}>
          <div style={{ position: 'relative', flex: 1, display: 'flex', alignItems: 'center' }}>
            <input
              style={{
                ...styles.input,
                marginBottom: 0, width: '100%', paddingRight: search ? '28px' : undefined, height: '32px', padding: '4px 10px', fontSize: '16px',
                ...(searchResults !== null ? { border: '1px solid #6366f1', background: '#1e1b4b', boxShadow: '0 0 0 2px rgba(99,102,241,0.15)' } : {}),
              }}
              placeholder="Search tasks..."
              value={search}
              onChange={e => setSearch(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && doSearch()}
            />
            {search && (
              <button
                type="button"
                aria-label="Clear search"
                onClick={() => { setSearch(''); setSearchResults(null); }}
                style={{
                  position: 'absolute',
                  right: '6px',
                  width: '22px',
                  height: '22px',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  borderRadius: '999px',
                  background: '#0b1220',
                  border: '1px solid #334155',
                  color: '#94a3b8',
                  cursor: 'pointer',
                  fontSize: '14px',
                  padding: 0,
                  lineHeight: 1,
                }}
                title="Clear search"
              >×</button>
            )}
          </div>
          <button style={{ ...styles.btn('secondary', false), ...(searchResults !== null ? { border: '1px solid #6366f1', color: '#a5b4fc', background: '#312e81' } : {}), ...(isMobile ? { padding: '3px 8px', minWidth: '32px' } : {}) }} onClick={doSearch} title="Search">
            {isMobile ? (
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><line x1="5" y1="12" x2="19" y2="12"/><polyline points="12 5 19 12 12 19"/></svg>
            ) : 'Search'}
          </button>
          {!isMobile && (
            <button style={{ ...styles.btn('secondary', false), ...(hasActiveFilters ? { border: '1px solid #6366f1', color: '#a5b4fc', background: '#312e81' } : {}), display: 'flex', alignItems: 'center', gap: isMobile ? '0' : '5px', ...(isMobile ? { padding: '3px 8px', minWidth: '32px' } : {}) }} onClick={() => setShowFilters(f => !f)} title="Filter">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"/></svg>
              {!isMobile && 'Filter'}
            </button>
          )}
        </div>
      )}
      {showSearchBar && (isMobile || showFilters) && (
        <div style={isMobile ? { display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '8px', padding: '8px 12px', alignItems: 'center' } : { display: 'flex', gap: '8px', padding: '8px 20px', flexWrap: 'wrap', alignItems: 'center' }}>
          <StyledSelect style={{ ...styles.select, marginBottom: 0, flex: isMobile ? '1 1 auto' : 'none', minWidth: isMobile ? 0 : '120px', width: isMobile ? '100%' : undefined, ...(isMobile ? { gridColumn: 'span 2' } : {}), padding: '6px 12px', fontSize: '16px', borderRadius: '4px', background: filterPriority ? '#3b82f611' : '#0f172a', border: `1px solid ${filterPriority ? '#3b82f644' : '#334155'}`, color: filterPriority ? '#93c5fd' : '#94a3b8', cursor: 'pointer', height: '32px', lineHeight: '1' }} value={filterPriority} onChange={e => setFilterPriority(e.target.value)}>
            <option value="">Any Priority</option>
            <option value="3">🔴 Critical</option>
            <option value="2">🟠 High</option>
            <option value="1">🟡 Medium</option>
            <option value="0">🟢 Low</option>
          </StyledSelect>
          <StyledSelect style={{ ...styles.select, marginBottom: 0, flex: isMobile ? '1 1 auto' : 'none', minWidth: isMobile ? 0 : '120px', width: isMobile ? '100%' : undefined, ...(isMobile ? { gridColumn: 'span 2' } : {}), padding: '6px 12px', fontSize: '16px', borderRadius: '4px', background: filterLabel ? '#3b82f611' : '#0f172a', border: `1px solid ${filterLabel ? '#3b82f644' : '#334155'}`, color: filterLabel ? '#93c5fd' : '#94a3b8', cursor: 'pointer', height: '32px', lineHeight: '1' }} value={filterLabel} onChange={e => setFilterLabel(e.target.value)}>
            <option value="">Any Label</option>
            {allLabelsSorted.map(l => (
              <option key={l} value={l}>{l}</option>
            ))}
          </StyledSelect>
          <StyledSelect style={{ ...styles.select, marginBottom: 0, flex: isMobile ? '1 1 auto' : 'none', minWidth: isMobile ? 0 : '120px', width: isMobile ? '100%' : undefined, ...(isMobile ? { gridColumn: 'span 2' } : {}), padding: '6px 12px', fontSize: '16px', borderRadius: '4px', background: filterAssignee ? '#3b82f611' : '#0f172a', border: `1px solid ${filterAssignee ? '#3b82f644' : '#334155'}`, color: filterAssignee ? '#93c5fd' : '#94a3b8', cursor: 'pointer', height: '32px', lineHeight: '1' }} value={filterAssignee} onChange={e => setFilterAssignee(e.target.value)}>
            <option value="">Any Assignee</option>
            {allAssignees.map(a => (
              <option key={a} value={a}>{a}</option>
            ))}
          </StyledSelect>
          <StyledSelect style={{ ...styles.select, marginBottom: 0, flex: isMobile ? '1 1 auto' : 'none', minWidth: isMobile ? 0 : '120px', width: isMobile ? '100%' : undefined, ...(isMobile ? { gridColumn: 'span 2' } : {}), padding: '6px 12px', fontSize: '16px', borderRadius: '4px', background: filterCreatedBy ? '#3b82f611' : '#0f172a', border: `1px solid ${filterCreatedBy ? '#3b82f644' : '#334155'}`, color: filterCreatedBy ? '#93c5fd' : '#94a3b8', cursor: 'pointer', height: '32px', lineHeight: '1' }} value={filterCreatedBy} onChange={e => setFilterCreatedBy(e.target.value)}>
            <option value="">Created By</option>
            {allCreators.map(c => (
              <option key={c} value={c}>{c}</option>
            ))}
          </StyledSelect>
          <button
            onClick={() => setShowArchivedTasks(v => !v)}
            style={{
              ...styles.select, marginBottom: 0, flex: isMobile ? '1 1 auto' : 'none',
              ...(isMobile ? { gridColumn: 'span 1', width: '100%' } : {}),
              padding: '6px 12px', cursor: 'pointer', whiteSpace: 'nowrap',
              background: showArchivedTasks ? '#6366f133' : '#0f172a',
              color: showArchivedTasks ? '#a5b4fc' : '#94a3b8',
              border: `1px solid ${showArchivedTasks ? '#6366f155' : '#334155'}`,
              borderRadius: '4px', fontSize: isMobile ? '16px' : '0.78rem',
              height: '32px', lineHeight: '1',
            }}
          >
            📦 {isMobile ? 'Archive' : 'Archived'} {showArchivedTasks ? '✓' : ''}
          </button>
          <button
            disabled={!hasActiveFilters && !showArchivedTasks}
            style={{
              ...styles.btnSmall,
              ...(isMobile ? { gridColumn: 'span 1', width: '100%' } : {}),
              ...(!hasActiveFilters && !showArchivedTasks ? { opacity: 0.4, cursor: 'not-allowed' } : {}),
            }}
            onClick={() => { setFilterPriority(''); setFilterLabel(''); setFilterAssignee(''); setFilterCreatedBy(''); setShowArchivedTasks(false); }}
          >
            {isMobile ? 'Clear' : 'Clear Filters'}
          </button>
        </div>
      )}

      <div style={styles.columnsContainer(isMobile)}>
        {columns.sort((a, b) => a.position - b.position).map(col => (
          <Column
            key={col.id}
            column={col}
            tasks={displayTasks}
            boardId={board.id}
            canEdit={canEdit}
            onRefresh={refreshAll}
            onBoardRefresh={onBoardRefresh}
            archived={archived}
            onClickTask={setSelectedTask}
            isMobile={isMobile}
            allColumns={columns}
            collapsed={collapsedColumns[col.id]}
            onToggleCollapse={() => toggleColumnCollapse(col.id)}
            tasksLoaded={tasksLoaded}
            onFullScreen={() => setFullScreenColumnId(col.id)}
          />
        ))}
        {canEdit && !archived && (
          addingColumn ? (
            <div style={{ ...styles.column(false, isMobile), minWidth: isMobile ? undefined : '200px', maxWidth: isMobile ? undefined : '200px', justifyContent: 'flex-start' }}>
              <input
                autoFocus
                style={{ background: '#1e293b', color: '#e2e8f0', border: '1px solid #3b82f6', borderRadius: '4px', padding: '6px 8px', fontSize: '16px', width: '100%' }}
                placeholder="Column name..."
                value={newColumnName}
                onChange={e => setNewColumnName(e.target.value)}
                onKeyDown={async (e) => {
                  if (e.key === 'Enter') {
                    const name = newColumnName.trim();
                    if (!name) return;
                    try {
                      await api.addColumn(board.id, { name });
                      setNewColumnName('');
                      setAddingColumn(false);
                      onBoardRefresh();
                    } catch (err) { alert(err.error || 'Failed to add column'); }
                  }
                  if (e.key === 'Escape') { setAddingColumn(false); setNewColumnName(''); }
                }}
                onBlur={() => { setAddingColumn(false); setNewColumnName(''); }}
              />
            </div>
          ) : (
            <div
              style={{
                minWidth: isMobile ? undefined : '60px', maxWidth: isMobile ? undefined : '60px',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
                cursor: 'pointer', color: '#64748b', fontSize: '1.5rem',
                borderRadius: '8px', border: '2px dashed #334155',
                minHeight: isMobile ? '50px' : undefined,
                transition: 'border-color .2s, color .2s',
              }}
              onClick={() => setAddingColumn(true)}
              onMouseEnter={e => { e.currentTarget.style.borderColor = '#3b82f6'; e.currentTarget.style.color = '#3b82f6'; }}
              onMouseLeave={e => { e.currentTarget.style.borderColor = '#334155'; e.currentTarget.style.color = '#64748b'; }}
              title="Add column"
            >+</div>
          )
        )}
        {columns.length === 0 && !addingColumn && (
          <div style={styles.empty}>No columns yet.</div>
        )}
      </div>

      {fullScreenColumnId && (() => {
        const fsCol = columns.find(c => c.id === fullScreenColumnId);
        return fsCol ? (
          <FullScreenColumnView
            column={fsCol}
            tasks={displayTasks}
            boardId={board.id}
            canEdit={canEdit}
            onRefresh={refreshAll}
            onClose={() => setFullScreenColumnId(null)}
            onClickTask={setSelectedTask}
            archived={archived}
          />
        ) : null;
      })()}

      {showCreate && (
        <CreateTaskModal
          boardId={board.id}
          columns={columns}
          onClose={() => setShowCreate(false)}
          onCreated={refreshAll}
          isMobile={isMobile}
          allLabels={allLabels}
          allAssignees={allAssignees}
        />
      )}

      {selectedTask && (
        <TaskDetailModal
          boardId={board.id}
          task={selectedTask}
          canEdit={canEdit}
          onClose={() => setSelectedTask(null)}
          onRefresh={refreshAll}
          isMobile={isMobile}
          allColumns={columns}
          allLabels={allLabels}
          allAssignees={allAssignees}
          quickDoneColumnId={board.quick_done_column_id}
          quickDoneAutoArchive={board.quick_done_auto_archive}
          quickReassignColumnId={board.quick_reassign_column_id}
          quickReassignTo={board.quick_reassign_to}
        />
      )}

      {showSettings && (
        <BoardSettingsModal
          board={board}
          canEdit={canEdit}
          onClose={() => setShowSettings(false)}
          onRefresh={onBoardRefresh}
          onBoardListRefresh={onBoardListRefresh}
          isMobile={isMobile}
        />
      )}

      {showActivity && (
        <ActivityPanel
          boardId={board.id}
          onClose={() => setShowActivity(false)}
          isMobile={isMobile}
          onOpenTask={(task) => { setSelectedTask(task); setShowActivity(false); }}
        />
      )}

      {/* Webhook management removed from UI (Jordan request). API still available. */}
    </div>
  );
}

function DirectBoardInput({ onOpen }) {
  const [boardId, setBoardId] = useState('');

  const submit = (e) => {
    e.preventDefault();
    const id = boardId.trim();
    if (!id) return;
    const match = id.match(/\/board\/([a-f0-9-]+)/i);
    onOpen(match ? match[1] : id);
    setBoardId('');
  };

  return (
    <form onSubmit={submit} style={{ display: 'flex', gap: '6px' }}>
      <input
        style={styles.directBoardInput}
        placeholder="Board ID or URL..."
        value={boardId}
        onChange={e => setBoardId(e.target.value)}
      />
      <button type="submit" style={styles.btnSmall}>Open</button>
    </form>
  );
}

// ---- Welcome / Public Boards Discovery ----

function WelcomePage({ onSelectBoard, onCreateBoard, isMobile }) {
  const [publicBoards, setPublicBoards] = useState([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');

  useEffect(() => {
    (async () => {
      try {
        const { data } = await api.listBoards(false);
        setPublicBoards(data.boards || data || []);
      } catch (err) {
        console.error('Failed to load public boards:', err);
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const filtered = searchQuery.trim()
    ? publicBoards.filter(b =>
        b.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        (b.description || '').toLowerCase().includes(searchQuery.toLowerCase())
      )
    : publicBoards;

  const totalTasks = publicBoards.reduce((sum, b) => sum + (b.task_count || 0), 0);

  return (
    <div style={{
      flex: 1, overflowY: 'auto', display: 'flex', flexDirection: 'column',
      alignItems: 'center', padding: isMobile ? '24px 16px' : '40px 24px',
    }}>
      {/* Hero */}
      <div style={{ textAlign: 'center', marginBottom: '32px', maxWidth: '520px' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '10px', marginBottom: '8px' }}>
          <img src="/logo.svg" alt="" style={{ width: '36px', height: '36px' }} />
          <h1 style={{ color: '#f1f5f9', fontSize: isMobile ? '1.6rem' : '2rem', fontWeight: 700, margin: 0 }}>Kanban</h1>
        </div>
        <p style={{ color: '#94a3b8', fontSize: '0.95rem', marginBottom: '6px' }}>
          Humans Not Required
        </p>
        <p style={{ color: '#64748b', fontSize: '0.83rem', lineHeight: '1.5', maxWidth: '400px', margin: '0 auto' }}>
          Agent-first project boards. No signup, no accounts — just create a board and share the link.
        </p>
        <button
          style={{
            ...styles.btn('primary', isMobile),
            marginTop: '16px',
            padding: '10px 24px',
            fontSize: '0.9rem',
          }}
          onClick={onCreateBoard}
        >
          + Create a Board
        </button>
      </div>

      {/* Stats bar */}
      {!loading && publicBoards.length > 0 && (
        <div style={{
          display: 'flex', gap: '24px', justifyContent: 'center', marginBottom: '24px',
          padding: '12px 20px', background: '#1e293b', borderRadius: '8px',
          border: '1px solid #334155',
        }}>
          <div style={{ textAlign: 'center' }}>
            <div style={{ color: '#a5b4fc', fontSize: '1.2rem', fontWeight: 700 }}>{publicBoards.length}</div>
            <div style={{ color: '#64748b', fontSize: '0.7rem', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
              Public Board{publicBoards.length !== 1 ? 's' : ''}
            </div>
          </div>
          <div style={{ width: '1px', background: '#334155' }} />
          <div style={{ textAlign: 'center' }}>
            <div style={{ color: '#22c55e', fontSize: '1.2rem', fontWeight: 700 }}>{totalTasks}</div>
            <div style={{ color: '#64748b', fontSize: '0.7rem', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
              Total Tasks
            </div>
          </div>
        </div>
      )}

      {/* Public Boards Section */}
      <div style={{ width: '100%', maxWidth: '800px' }}>
        <div style={{
          display: 'flex', justifyContent: 'space-between', alignItems: 'center',
          marginBottom: '12px', gap: '12px', flexWrap: 'wrap',
        }}>
          <h2 style={{ color: '#e2e8f0', fontSize: '1rem', fontWeight: 600, margin: 0 }}>
            🌐 Public Boards
          </h2>
          {publicBoards.length > 3 && (
            <div style={{ position: 'relative', flex: isMobile ? '1 1 100%' : '0 1 240px' }}>
              <input
                style={{
                  ...styles.input, marginBottom: 0, width: '100%',
                  height: '32px', padding: '4px 10px', fontSize: '16px',
                  paddingRight: searchQuery ? '28px' : undefined,
                }}
                placeholder="Search boards..."
                value={searchQuery}
                onChange={e => setSearchQuery(e.target.value)}
              />
              {searchQuery && (
                <button
                  onClick={() => setSearchQuery('')}
                  style={{
                    position: 'absolute', right: '6px', top: '50%', transform: 'translateY(-50%)',
                    width: '20px', height: '20px', display: 'flex', alignItems: 'center', justifyContent: 'center',
                    borderRadius: '999px', background: '#0b1220', border: '1px solid #334155',
                    color: '#94a3b8', cursor: 'pointer', fontSize: '13px', padding: 0, lineHeight: 1,
                  }}
                >×</button>
              )}
            </div>
          )}
        </div>

        {loading ? (
          <div style={{ color: '#64748b', textAlign: 'center', padding: '40px 0', fontSize: '0.85rem' }}>
            Loading boards...
          </div>
        ) : filtered.length === 0 ? (
          <div style={{
            textAlign: 'center', padding: '32px 16px', color: '#475569',
            background: '#1a2332', borderRadius: '8px', border: '1px solid #334155',
          }}>
            {searchQuery
              ? <p style={{ fontSize: '0.85rem' }}>No boards matching "{searchQuery}"</p>
              : (
                <>
                  <p style={{ fontSize: '0.9rem', marginBottom: '4px' }}>No public boards yet.</p>
                  <p style={{ fontSize: '0.8rem' }}>Create the first one!</p>
                </>
              )
            }
          </div>
        ) : (
          <div style={{
            display: 'grid',
            gridTemplateColumns: isMobile ? '1fr' : 'repeat(auto-fill, minmax(240px, 1fr))',
            gap: '12px',
          }}>
            {filtered.map(board => (
              <div
                key={board.id}
                onClick={() => onSelectBoard(board.id)}
                style={{
                  background: '#1a2332', border: '1px solid #334155', borderRadius: '8px',
                  padding: '16px', cursor: 'pointer',
                  transition: 'border-color 0.15s, background 0.15s',
                }}
                onMouseEnter={e => { e.currentTarget.style.borderColor = '#6366f1'; e.currentTarget.style.background = '#1e293b'; }}
                onMouseLeave={e => { e.currentTarget.style.borderColor = '#334155'; e.currentTarget.style.background = '#1a2332'; }}
              >
                <h3 style={{
                  color: '#e2e8f0', fontSize: '0.95rem', fontWeight: 600,
                  margin: '0 0 6px 0',
                  overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                }}>
                  {board.name}
                </h3>
                {board.description && (
                  <p style={{
                    color: '#94a3b8', fontSize: '0.78rem', margin: '0 0 10px 0',
                    lineHeight: '1.4',
                    display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical',
                    overflow: 'hidden',
                  }}>
                    {board.description}
                  </p>
                )}
                <div style={{ display: 'flex', gap: '12px', alignItems: 'center', fontSize: '0.72rem', color: '#64748b' }}>
                  <span title="Tasks">📋 {board.task_count} task{board.task_count !== 1 ? 's' : ''}</span>
                  <span title="Created">{formatTimeAgo(board.created_at)}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Open by ID section */}
      <div style={{
        width: '100%', maxWidth: '800px', marginTop: '32px',
        padding: '16px 20px', background: '#1e293b', borderRadius: '8px',
        border: '1px solid #334155',
      }}>
        <div style={{
          fontSize: '0.8rem', fontWeight: 600, color: '#94a3b8',
          marginBottom: '8px',
        }}>
          Open a board by ID or URL
        </div>
        <DirectBoardInput onOpen={onSelectBoard} />
      </div>
    </div>
  );
}

function App() {
  const { isMobile, isCompact } = useBreakpoint();
  const collapseSidebar = isCompact; // collapse sidebar on mobile + tablet
  const [myBoards, setMyBoards] = useState(() => api.getMyBoards());
  const [selectedBoardId, setSelectedBoardId] = useState(null);
  const [boardDetail, setBoardDetail] = useState(null);
  const [showCreateBoard, setShowCreateBoard] = useState(false);
  const [loadError, setLoadError] = useState(null);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [sseStatus, setSseStatus] = useState('initial');

  const refreshMyBoards = useCallback(() => setMyBoards(api.getMyBoards()), []);

  useEffect(() => {
    const { boardId, key } = api.extractKeyFromUrl();
    if (boardId && key) {
      api.setBoardKey(boardId, key);
      api.cleanKeyFromUrl();
      setSelectedBoardId(boardId);
    } else if (boardId) {
      setSelectedBoardId(boardId);
    }
  }, []);

  const loadBoardDetail = useCallback(async (boardId) => {
    const id = boardId || selectedBoardId;
    if (!id) { setBoardDetail(null); setLoadError(null); setSseStatus('initial'); return; }
    setLoadError(null);
    try {
      const { data } = await api.getBoard(id);
      setBoardDetail(data);
      // Auto-add to My Boards when successfully loaded
      api.addMyBoard(id, data.name || 'Untitled Board');
      refreshMyBoards();
    } catch (err) {
      console.error('Failed to load board:', err);
      setLoadError(err.status === 404 ? 'Board not found.' : 'Failed to load board.');
      setBoardDetail(null);
    }
  }, [selectedBoardId, refreshMyBoards]);

  useEffect(() => {
    if (!selectedBoardId) { setBoardDetail(null); setLoadError(null); return; }
    loadBoardDetail(selectedBoardId);
  }, [selectedBoardId, loadBoardDetail]);

  // keyVersion bumps when a manage key is added/removed, forcing canEdit re-derive
  const [keyVersion, setKeyVersion] = useState(0);
  const canEdit = selectedBoardId ? api.hasBoardKey(selectedBoardId) : false;
  // eslint-disable-next-line no-unused-vars
  void keyVersion; // Referenced to ensure React includes it in dependency tracking

  const handleKeyUpgraded = useCallback(() => {
    setKeyVersion(v => v + 1);
    if (selectedBoardId) loadBoardDetail(selectedBoardId);
  }, [selectedBoardId, loadBoardDetail]);

  const handleBoardCreated = (newBoardId) => {
    if (newBoardId) setSelectedBoardId(newBoardId);
  };

  const handleOpenDirect = (boardId) => {
    setSelectedBoardId(boardId);
    setSidebarOpen(false);
  };

  const handleSelectBoard = (boardId) => {
    setSelectedBoardId(boardId);
    if (collapseSidebar) setSidebarOpen(false);
  };

  const handleRemoveMyBoard = (e, boardId) => {
    e.stopPropagation();
    api.removeMyBoard(boardId);
    refreshMyBoards();
    if (selectedBoardId === boardId) {
      setSelectedBoardId(null);
      setBoardDetail(null);
    }
  };

  return (
    <div style={styles.app(isMobile)}>
      <div style={styles.header(isMobile)}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flex: isCompact ? '1 1 0' : undefined }}>
          {collapseSidebar && (
            <button
              style={styles.menuBtn}
              onClick={() => setSidebarOpen(o => !o)}
              aria-label={sidebarOpen ? 'Close sidebar' : 'Open sidebar'}
            >
              <svg width="18" height="18" viewBox="0 0 18 18" fill="none" style={{ display: 'block' }}>
                <rect
                  y={sidebarOpen ? 8 : 2} width="18" height="2" rx="1" fill="currentColor"
                  style={{ transition: 'all 0.25s ease', transformOrigin: 'center',
                    transform: sidebarOpen ? 'rotate(45deg)' : 'rotate(0)' }}
                />
                <rect
                  y="8" width="18" height="2" rx="1" fill="currentColor"
                  style={{ transition: 'opacity 0.2s ease', opacity: sidebarOpen ? 0 : 1 }}
                />
                <rect
                  y={sidebarOpen ? 8 : 14} width="18" height="2" rx="1" fill="currentColor"
                  style={{ transition: 'all 0.25s ease', transformOrigin: 'center',
                    transform: sidebarOpen ? 'rotate(-45deg)' : 'rotate(0)' }}
                />
              </svg>
            </button>
          )}
          {/* On compact screens (tablet/mobile): identity badge next to hamburger (left side) */}
          {isCompact && selectedBoardId && canEdit && (
            <IdentityBadge isMobile={isMobile} />
          )}
          {/* On desktop (non-compact): logo stays left */}
          {!isCompact && (
            <div style={styles.logo} onClick={() => { setSelectedBoardId(null); setBoardDetail(null); }}>
              <img src="/logo.svg" alt="" style={styles.logoImg} />
              Kanban
            </div>
          )}
        </div>
        {/* On tablet: logo centered */}
        {isCompact && (
          <div style={{ ...styles.logo, flex: '0 0 auto' }} onClick={() => { setSelectedBoardId(null); setBoardDetail(null); }}>
            <img src="/logo.svg" alt="" style={styles.logoImg} />
            Kanban
          </div>
        )}
        <div style={{ ...styles.headerRight, flex: isCompact ? '1 1 0' : undefined, justifyContent: isCompact ? 'flex-end' : undefined }}>
          {/* On desktop: live indicator pill to the left of the username */}
          {!isCompact && selectedBoardId && <LiveIndicator status={sseStatus} isMobile={false} />}
          {/* On desktop: identity badge stays on right */}
          {!isCompact && selectedBoardId && canEdit && (
            <IdentityBadge isMobile={isMobile} />
          )}
          {/* On mobile/tablet: compact dot indicator */}
          {isCompact && selectedBoardId && <LiveIndicator status={sseStatus} isMobile={true} />}
          {selectedBoardId && (
            <AccessIndicator boardId={selectedBoardId} canEdit={canEdit} isMobile={isMobile} onKeyUpgraded={handleKeyUpgraded} />
          )}
        </div>
      </div>

      <div style={styles.main(isMobile)}>
        {/* Sidebar overlay for mobile */}
        {collapseSidebar && sidebarOpen && (
          <div style={styles.sidebarOverlay} onClick={() => setSidebarOpen(false)} />
        )}

        <div style={styles.sidebar(collapseSidebar, sidebarOpen)}>
          <div style={styles.sidebarHeader}>
            <span>My Boards</span>
            <button style={styles.btnSmall} onClick={() => { setShowCreateBoard(true); setSidebarOpen(false); }}>+ New</button>
          </div>
          {myBoards.map(b => {
            const hasKey = api.hasBoardKey(b.id);
            return (
              <div
                key={b.id}
                style={{
                  ...styles.boardItem(selectedBoardId === b.id),
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                }}
                onClick={() => handleSelectBoard(b.id)}
              >
                <span title={hasKey ? 'Full access' : 'View only'} style={{ fontSize: '0.7rem', flexShrink: 0, opacity: 0.7 }}>
                  {hasKey ? '✏️' : '👁'}
                </span>
                <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{b.name}</span>
                <button
                  onClick={(e) => handleRemoveMyBoard(e, b.id)}
                  title="Remove from My Boards"
                  style={{
                    background: 'none',
                    border: 'none',
                    color: '#64748b',
                    cursor: 'pointer',
                    padding: '0 2px',
                    fontSize: '0.7rem',
                    flexShrink: 0,
                    lineHeight: 1,
                    opacity: 0.5,
                    transition: 'opacity 0.15s',
                  }}
                  onMouseEnter={e => e.currentTarget.style.opacity = '1'}
                  onMouseLeave={e => e.currentTarget.style.opacity = '0.5'}
                >
                  ✕
                </button>
              </div>
            );
          })}
          {myBoards.length === 0 && (
            <div style={{ ...styles.empty, padding: '20px 16px', fontSize: '0.8rem' }}>
              No boards yet. Create one or open by ID.
            </div>
          )}

          <div style={{ borderTop: '1px solid #334155', marginTop: 'auto', padding: '12px' }}>
            <DirectBoardInput onOpen={handleOpenDirect} />
            <button
              onClick={() => { setSelectedBoardId(null); setBoardDetail(null); if (collapseSidebar) setSidebarOpen(false); }}
              style={{
                background: 'transparent',
                color: '#a5b4fc',
                border: '1px solid #334155',
                borderRadius: '4px',
                padding: '5px 10px',
                fontSize: '0.75rem',
                cursor: 'pointer',
                width: '100%',
                marginTop: '8px',
                height: '32px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                gap: '6px',
                transition: 'background 0.15s, border-color 0.15s',
              }}
              onMouseEnter={e => { e.currentTarget.style.background = '#6366f122'; e.currentTarget.style.borderColor = '#6366f155'; }}
              onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.borderColor = '#334155'; }}
            >
              🌐 Browse Public Boards
            </button>
          </div>
        </div>

        {boardDetail ? (
          <BoardView board={boardDetail} canEdit={canEdit} onRefresh={() => loadBoardDetail()} onBoardRefresh={() => loadBoardDetail()} onBoardListRefresh={() => {}} isMobile={isMobile} onSseStatusChange={setSseStatus} />
        ) : loadError ? (
          <div style={{ ...styles.boardContent(isMobile), ...styles.empty, justifyContent: 'center', display: 'flex', alignItems: 'center' }}>
            <div>
              <p style={{ fontSize: '1.1rem', marginBottom: '8px', color: '#ef4444' }}>{loadError}</p>
              <p style={{ fontSize: '0.85rem' }}>Check the board ID and try again.</p>
            </div>
          </div>
        ) : (
          <WelcomePage
            onSelectBoard={handleOpenDirect}
            onCreateBoard={() => setShowCreateBoard(true)}
            isMobile={isMobile}
          />
        )}
      </div>

      {showCreateBoard && (
        <CreateBoardModal
          onClose={() => setShowCreateBoard(false)}
          onCreated={handleBoardCreated}
          isMobile={isMobile}
        />
      )}
      <footer style={{ textAlign: 'center', padding: '8px 16px', fontSize: '0.65rem', color: '#475569', flexShrink: 0 }}>
        Made for AI, by AI.{' '}
        <a href="https://github.com/Humans-Not-Required" target="_blank" rel="noopener noreferrer" style={{ color: '#6366f1', textDecoration: 'none' }}>Humans not required</a>.
      </footer>
    </div>
  );
}

export default App;
