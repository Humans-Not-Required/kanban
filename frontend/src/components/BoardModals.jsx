import { useState, useEffect, useCallback } from "react";
import * as api from "../api";
import styles from "../styles";
import { parseUTC, copyToClipboard, WEBHOOK_EVENTS } from "../utils";
import { useEscapeKey } from "../hooks";
import StyledSelect from "./StyledSelect";

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


// ---- Webhook Manager ----
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

export { CreateBoardModal, BoardSettingsModal, WebhookManagerModal };
