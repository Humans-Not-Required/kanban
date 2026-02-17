import { useState } from 'react';

export default function AutocompleteInput({ value, onChange, suggestions, placeholder, style, isCommaList }) {
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [focusedIdx, setFocusedIdx] = useState(-1);

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
