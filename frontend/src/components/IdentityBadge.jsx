import { useState } from 'react';
import * as api from '../api';
import styles from '../styles';

export default function IdentityBadge({ isMobile }) {
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
