import { useState, useEffect, useCallback, useRef } from "react";
import * as api from "../api";
import styles from "../styles";
import { parseUTC, normalizeLabels, priorityColor, priorityLabel, renderWithMentions } from "../utils";
import { useEscapeKey } from "../hooks";
import AutocompleteInput from "./AutocompleteInput";
import StyledSelect from "./StyledSelect";
import PriorityToggle from "./PriorityToggle";
import { MoveTaskDropdown } from "./TaskCard";

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
  const [linkCopied, setLinkCopied] = useState(false);

  const handleCopyLink = () => {
    const url = new URL(window.location.href);
    url.searchParams.set('task', task.id);
    url.searchParams.delete('key'); // never share manage key in task links
    navigator.clipboard.writeText(url.toString()).then(() => {
      setLinkCopied(true);
      setTimeout(() => setLinkCopied(false), 1500);
    });
  };
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
      await onRefresh();
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
      await onRefresh();
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
      await onRefresh();
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
        await onRefresh();
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
      await onRefresh();
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
                <button
                  style={{ ...styles.btnIcon, color: linkCopied ? '#22c55e' : '#94a3b8' }}
                  onClick={handleCopyLink}
                  title="Copy task link"
                >{linkCopied ? '✓' : '🔗'}</button>
                <button style={styles.btnClose} onClick={onClose}>×</button>
              </div>
            ) : (
              <button style={{ ...styles.btnClose, marginLeft: '8px', flexShrink: 0 }} onClick={onClose}>×</button>
            )}
          </div>
          {/* Row 2: Action buttons on mobile (below title) */}
          {isMobile && !editing && (
            <div style={{ display: 'flex', gap: '6px', justifyContent: 'flex-end', marginTop: '10px', flexWrap: 'wrap' }}>
              {canEdit && reassignColumn && !isAlreadyInReassignCol && !isArchived && (
                <button
                  style={{ ...styles.btnIcon, background: '#f59e0b22', borderColor: '#f59e0b44', color: '#fbbf24' }}
                  onClick={handleQuickReassign}
                  disabled={reassigning}
                  title={`Move to ${reassignColumn.name}${quickReassignTo ? ` → ${quickReassignTo}` : ''}`}
                >{reassigning ? '⏳' : '↩'}</button>
              )}
              {canEdit && doneColumn && !isAlreadyDone && !isArchived && (
                <button
                  style={{ ...styles.btnIcon, background: '#22c55e22', borderColor: '#22c55e44', color: '#4ade80' }}
                  onClick={handleMarkDone}
                  disabled={markingDone}
                  title={`Mark done${quickDoneAutoArchive ? ' & archive' : ''} → ${doneColumn.name}`}
                >{markingDone ? '⏳' : '✓'}</button>
              )}
              {canEdit && (
                <button
                  style={styles.btnIcon}
                  onClick={handleArchiveToggle}
                  disabled={archiving}
                  title={isArchived ? 'Unarchive task' : 'Archive task'}
                >{archiving ? '⏳' : isArchived ? '📤' : '📦'}</button>
              )}
              {canEdit && (
                <button
                  style={styles.btnIcon}
                  onClick={() => setEditing(true)}
                  title="Edit task"
                >✏️</button>
              )}
              <button
                style={{ ...styles.btnIcon, color: linkCopied ? '#22c55e' : '#94a3b8' }}
                onClick={handleCopyLink}
                title="Copy task link"
              >{linkCopied ? '✓' : '🔗'}</button>
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
                onMoved={async () => { setShowMove(false); await onRefresh(); onClose(); }}
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


export { CreateTaskModal, TaskDetailModal };
