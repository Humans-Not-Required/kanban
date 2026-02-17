import { useState } from "react";
import * as api from "../api";
import styles from "../styles";

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


export { SharePopover, AccessIndicator };
