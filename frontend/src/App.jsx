import { useState, useEffect, useCallback } from 'react';
import * as api from './api';

// ---- Escape key hook ----
function useEscapeKey(onClose) {
  useEffect(() => {
    const handler = (e) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
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
  app: { height: '100vh', display: 'flex', flexDirection: 'column', overflow: 'hidden' },
  header: (mobile) => ({
    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
    padding: mobile ? '8px 10px' : '12px 20px', background: '#1e293b',
    borderBottom: '1px solid #334155',
    minHeight: mobile ? '40px' : '48px', overflow: 'hidden',
    gap: '8px',
  }),
  logo: { fontSize: '1.2rem', fontWeight: 700, color: '#f1f5f9', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '8px', flexShrink: 0 },
  logoImg: { width: '24px', height: '24px' },
  headerRight: { display: 'flex', alignItems: 'center', gap: '6px', fontSize: '0.85rem', overflow: 'hidden', flexShrink: 1, minWidth: 0 },
  menuBtn: {
    background: 'transparent', border: '1px solid #475569', color: '#cbd5e1',
    padding: '6px 8px', borderRadius: '6px', cursor: 'pointer', fontSize: '1.1rem',
    lineHeight: 1, transition: 'background 0.15s, border-color 0.15s',
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
    flex: 1, display: 'flex',
    flexDirection: mobile ? 'column' : 'row',
    overflow: 'hidden', position: 'relative',
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
  boardContent: { flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' },
  boardHeader: (mobile) => ({
    padding: mobile ? '12px' : '16px 20px',
    display: 'flex', alignItems: mobile ? 'flex-start' : 'center',
    justifyContent: 'space-between', borderBottom: '1px solid #1e293b',
    flexDirection: mobile ? 'column' : 'row', gap: mobile ? '8px' : '0',
  }),
  boardTitle: (mobile) => ({
    fontSize: mobile ? '1.1rem' : '1.3rem', fontWeight: 700, color: '#f1f5f9',
  }),
  columnsContainer: (mobile) => ({
    flex: 1, display: 'flex',
    flexDirection: mobile ? 'column' : 'row',
    gap: mobile ? '12px' : '16px',
    padding: mobile ? '12px' : '16px 20px',
    overflowX: mobile ? 'hidden' : 'auto',
    overflowY: mobile ? 'auto' : 'hidden',
    alignItems: mobile ? 'stretch' : 'flex-start',
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
    maxHeight: mobile ? 'none' : 'calc(100vh - 200px)',
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
  }),
  btnSmall: {
    background: 'transparent', border: '1px solid #334155', color: '#94a3b8',
    padding: '3px 8px', borderRadius: '4px', cursor: 'pointer', fontSize: '0.75rem',
  },
  modal: (mobile) => ({
    position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.6)',
    display: 'flex', alignItems: mobile ? 'stretch' : 'flex-start', justifyContent: 'center', zIndex: 100,
    padding: mobile ? '0' : '12px',
    paddingTop: mobile ? '0' : '8vh',
  }),
  modalContent: (mobile) => ({
    background: '#1e293b', border: mobile ? 'none' : '1px solid #334155', borderRadius: mobile ? '0' : '8px',
    padding: mobile ? '16px' : '24px',
    width: mobile ? '100%' : '480px', maxWidth: '100%',
    maxHeight: mobile ? '100vh' : '80vh', height: mobile ? '100vh' : 'auto', overflow: 'auto',
  }),
  modalContentWide: (mobile) => ({
    background: '#1e293b', border: mobile ? 'none' : '1px solid #334155', borderRadius: mobile ? '0' : '8px',
    padding: mobile ? '16px' : '24px',
    width: mobile ? '100%' : '560px', maxWidth: '100%',
    maxHeight: mobile ? '100vh' : '80vh', height: mobile ? '100vh' : 'auto', overflow: 'auto',
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
    flex: 1,
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
            padding: '3px 8px', borderRadius: '4px', fontSize: '0.8rem',
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
      onClick={() => { if (!dragging) onClickTask(task); }}
    >
      <div style={styles.cardTitle}>{task.title}</div>
      <div style={styles.cardMeta}>
        <span style={{ color: priorityColor(task.priority) }}>{priorityLabel(task.priority)}</span>
        {task.assigned_to && <span>→ {task.assigned_to}</span>}
        {task.claimed_by && <span>🔒 {task.claimed_by}</span>}
        {task.due_at && <span>📅 {new Date(task.due_at).toLocaleDateString()}</span>}
        {task.completed_at && <span>✅</span>}
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

function Column({ column, tasks, boardId, canEdit, onRefresh, onBoardRefresh, archived, onClickTask, isMobile, allColumns, collapsed: externalCollapsed, onToggleCollapse }) {
  const [dragOver, setDragOver] = useState(false);
  const [internalCollapsed, setInternalCollapsed] = useState(false);
  const collapsed = isMobile ? internalCollapsed : (externalCollapsed || false);
  const toggleCollapse = isMobile ? () => setInternalCollapsed(c => !c) : onToggleCollapse;
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(column.name);
  const [showMenu, setShowMenu] = useState(false);
  const colTasks = tasks.filter(t => t.column_id === column.id)
    .sort((a, b) => (a.position ?? 999) - (b.position ?? 999));

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
          cursor: 'pointer', maxHeight: 'calc(100vh - 200px)', overflow: 'hidden',
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
          maxHeight: 'calc(100vh - 260px)',
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
            style={{ background: '#1e293b', color: '#e2e8f0', border: '1px solid #3b82f6', borderRadius: '4px', padding: '2px 6px', fontSize: '0.85rem', fontWeight: 600, width: '100%' }}
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
          <div style={{
            position: 'absolute', top: '100%', right: 0, zIndex: 50,
            background: '#1e293b', border: '1px solid #334155', borderRadius: '6px',
            padding: '4px 0', minWidth: '140px', boxShadow: '0 4px 12px rgba(0,0,0,.4)',
          }}>
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
              >⬅️ Move Left</button>
            )}
            {!isLast && (
              <button
                style={{ display: 'block', width: '100%', textAlign: 'left', padding: '6px 12px', background: 'none', border: 'none', color: '#e2e8f0', cursor: 'pointer', fontSize: '0.8rem' }}
                onClick={() => { handleMoveColumn(1); setShowMenu(false); }}
                onMouseEnter={e => e.target.style.background = '#334155'}
                onMouseLeave={e => e.target.style.background = 'none'}
              >➡️ Move Right</button>
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
          {colTasks.map(t => (
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
        </div>
      )}
    </div>
  );
}

function CreateTaskModal({ boardId, columns, onClose, onCreated, isMobile, allLabels, allAssignees }) {
  useEscapeKey(onClose);
  const [title, setTitle] = useState('');
  const [desc, setDesc] = useState('');
  const [priority, setPriority] = useState(1);
  const [columnId, setColumnId] = useState(columns[0]?.id || '');
  const [labels, setLabels] = useState('');
  const [assignedTo, setAssignedTo] = useState('');
  const [loading, setLoading] = useState(false);

  const submitTask = async () => {
    if (!title.trim() || loading) return;
    setLoading(true);
    try {
      await api.createTask(boardId, {
        title: title.trim(),
        description: desc.trim() || '',
        priority: Number(priority),
        column_id: columnId,
        labels: labels.trim() ? labels.split(',').map(l => l.trim()).filter(Boolean) : [],
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

  // Shift+Enter submits from anywhere in the modal
  useEffect(() => {
    const handler = (e) => {
      if (e.shiftKey && e.key === 'Enter') { e.preventDefault(); submitTask(); }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  });

  return (
    <div style={styles.modal(isMobile)} onClick={onClose}>
      <div style={styles.modalContent(isMobile)} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ marginBottom: '16px', color: '#f1f5f9' }}>New Task</h3>
        <form onSubmit={submit}>
          <input style={styles.input} placeholder="Title" value={title} onChange={e => setTitle(e.target.value)} autoFocus />
          <textarea style={styles.textarea} placeholder="Description (optional)" value={desc} onChange={e => setDesc(e.target.value)} />
          <div style={{ display: 'flex', gap: '10px', marginBottom: '10px' }}>
            <select style={styles.select} value={priority} onChange={e => setPriority(Number(e.target.value))}>
              <option value={3}>Critical</option>
              <option value={2}>High</option>
              <option value={1}>Medium</option>
              <option value={0}>Low</option>
            </select>
            <select style={styles.select} value={columnId} onChange={e => setColumnId(e.target.value)}>
              {columns.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
            </select>
          </div>
          <AutocompleteInput style={styles.input} placeholder="Labels (comma-separated)" value={labels} onChange={setLabels} suggestions={allLabels || []} isCommaList />
          <AutocompleteInput style={styles.input} placeholder="Assigned to (optional)" value={assignedTo} onChange={setAssignedTo} suggestions={allAssignees || []} />
          <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
            <button type="button" style={styles.btn('secondary', isMobile)} onClick={onClose}>Cancel</button>
            <button type="submit" style={styles.btn('primary', isMobile)} disabled={loading || !title.trim()}>
              {loading ? 'Creating...' : 'Create Task'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

function TaskDetailModal({ boardId, task, canEdit, onClose, onRefresh, isMobile, allColumns, allLabels, allAssignees }) {
  useEscapeKey(onClose);
  const [events, setEvents] = useState([]);
  const [comment, setComment] = useState('');
  const [actorName, setActorName] = useState(() => api.getDisplayName());
  const [loadingEvents, setLoadingEvents] = useState(true);
  const [posting, setPosting] = useState(false);
  const [showMove, setShowMove] = useState(false);
  const [editing, setEditing] = useState(false);
  const [editTitle, setEditTitle] = useState(task.title);
  const [editDesc, setEditDesc] = useState(task.description || '');
  const [editPriority, setEditPriority] = useState(task.priority);
  const [editLabels, setEditLabels] = useState((task.labels || []).join(', '));
  const [editAssigned, setEditAssigned] = useState(task.assigned_to || '');
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);

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
    setSaving(true);
    try {
      const updates = {};
      if (editTitle.trim() !== task.title) updates.title = editTitle.trim();
      if (editDesc.trim() !== (task.description || '')) updates.description = editDesc.trim();
      if (editPriority !== task.priority) updates.priority = editPriority;
      const newLabels = editLabels.trim() ? editLabels.split(',').map(l => l.trim()).filter(Boolean) : [];
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

  const formatTime = (ts) => {
    try {
      const d = new Date(ts);
      return d.toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
    } catch { return ts; }
  };

  const eventLabel = (evt) => {
    switch (evt.event_type) {
      case 'created': return '🆕 Created';
      case 'moved': return `➡️ Moved to ${evt.data?.to_column || 'column'}`;
      case 'claimed': return `🔒 Claimed`;
      case 'released': return `🔓 Released`;
      case 'updated': return '✏️ Updated';
      case 'assigned': return `👤 Assigned to ${evt.data?.assigned_to || 'someone'}`;
      default: return evt.event_type;
    }
  };

  return (
    <div style={styles.modal(isMobile)} onClick={onClose}>
      <div style={styles.modalContentWide(isMobile)} onClick={(e) => e.stopPropagation()}>
        {/* Task header */}
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '16px' }}>
          <div style={{ flex: 1, minWidth: 0 }}>
            {editing ? (
              <input
                style={{ ...styles.input, fontSize: '1.1rem', fontWeight: 600, marginBottom: '6px' }}
                value={editTitle}
                onChange={e => setEditTitle(e.target.value)}
                autoFocus
              />
            ) : (
              <h3 style={{ color: '#f1f5f9', marginBottom: '6px', fontSize: isMobile ? '1rem' : '1.17rem' }}>{task.title}</h3>
            )}
            {!editing && (
              <div style={styles.cardMeta}>
                <span style={{ color: priorityColor(task.priority) }}>
                  {priorityLabel(task.priority)}
                </span>
                {task.assigned_to && <span>→ {task.assigned_to}</span>}
                {task.claimed_by && <span>🔒 {task.claimed_by}</span>}
                {task.column_name && <span>⬜ {task.column_name}</span>}
                {task.created_by && task.created_by !== 'anonymous' && <span>by {task.created_by}</span>}
              </div>
            )}
          </div>
          <div style={{ display: 'flex', gap: '4px', marginLeft: '8px', flexShrink: 0 }}>
            {canEdit && !editing && (
              <button
                style={{ ...styles.btnSmall, padding: '6px 10px', fontSize: '0.8rem' }}
                onClick={() => setEditing(true)}
                title="Edit task"
              >✏️</button>
            )}
            <button
              style={{ ...styles.btnSmall, padding: '6px 10px', fontSize: '1rem', lineHeight: 1 }}
              onClick={onClose}
            >×</button>
          </div>
        </div>

        {/* Edit form */}
        {editing && (
          <div style={{ marginBottom: '16px', padding: '12px', background: '#0f172a', borderRadius: '6px', border: '1px solid #6366f133' }}>
            <textarea
              style={{ ...styles.textarea, minHeight: '60px' }}
              placeholder="Description (optional)"
              value={editDesc}
              onChange={e => setEditDesc(e.target.value)}
            />
            <div style={{ display: 'flex', gap: '10px', marginBottom: '10px' }}>
              <select style={styles.select} value={editPriority} onChange={e => setEditPriority(Number(e.target.value))}>
                <option value={3}>Critical</option>
                <option value={2}>High</option>
                <option value={1}>Medium</option>
                <option value={0}>Low</option>
              </select>
            </div>
            <AutocompleteInput
              style={styles.input}
              placeholder="Labels (comma-separated)"
              value={editLabels}
              onChange={setEditLabels}
              suggestions={allLabels || []}
              isCommaList
            />
            <AutocompleteInput
              style={styles.input}
              placeholder="Assigned to (optional)"
              value={editAssigned}
              onChange={setEditAssigned}
              suggestions={allAssignees || []}
            />
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
                  disabled={saving || !editTitle.trim()}
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
            <div style={{ maxHeight: '200px', overflowY: 'auto', marginBottom: '12px' }}>
              {comments.map(evt => (
                <div key={evt.id} style={{ marginBottom: '10px', padding: '8px 10px', background: '#0f172a', borderRadius: '6px', border: '1px solid #334155' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '4px' }}>
                    <span style={{ fontSize: '0.78rem', fontWeight: 600, color: '#a5b4fc' }}>{evt.actor || 'anonymous'}</span>
                    <span style={{ fontSize: '0.7rem', color: '#475569' }}>{formatTime(evt.created_at)}</span>
                  </div>
                  <div style={{ fontSize: '0.83rem', color: '#cbd5e1', whiteSpace: 'pre-wrap' }}>
                    {evt.data?.message || ''}
                  </div>
                </div>
              ))}
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
  useEscapeKey(onClose);
  const [name, setName] = useState('');
  const [desc, setDesc] = useState('');
  const [columns, setColumns] = useState('To Do, In Progress, Done');
  const [isPublic, setIsPublic] = useState(false);
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState(null);
  const [copied, setCopied] = useState(null);

  const submit = async (e) => {
    e.preventDefault();
    if (!name.trim()) return;
    setLoading(true);
    try {
      const cols = columns.split(',').map(c => c.trim()).filter(Boolean);
      const { data } = await api.createBoard({
        name: name.trim(),
        description: desc.trim() || undefined,
        is_public: isPublic,
        columns: cols.map((n, i) => ({
          name: n,
          position: i,
          is_done_column: i === cols.length - 1,
        })),
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
    <div style={styles.modal(isMobile)} onClick={onClose}>
      <div style={styles.modalContent(isMobile)} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ marginBottom: '16px', color: '#f1f5f9' }}>New Board</h3>
        <form onSubmit={submit}>
          <input style={styles.input} placeholder="Board Name" value={name} onChange={e => setName(e.target.value)} autoFocus />
          <textarea style={styles.textarea} placeholder="Description (optional)" value={desc} onChange={e => setDesc(e.target.value)} />
          <input style={styles.input} placeholder="Columns (comma-separated)" value={columns} onChange={e => setColumns(e.target.value)} />
          <p style={{ fontSize: '0.73rem', color: '#64748b', marginBottom: '12px' }}>
            Last column is automatically marked as "done" column.
          </p>
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

function BoardSettingsModal({ board, canEdit, onClose, onRefresh, isMobile }) {
  useEscapeKey(onClose);
  const [name, setName] = useState(board.name);
  const [description, setDescription] = useState(board.description || '');
  const [isPublic, setIsPublic] = useState(board.is_public || false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');

  const handleSave = async () => {
    setError('');
    if (!name.trim()) { setError('Board name is required'); return; }
    setSaving(true);
    try {
      await api.updateBoard(board.id, {
        name: name.trim(),
        description: description.trim(),
        is_public: isPublic,
      });
      onRefresh();
      onClose();
    } catch (err) {
      setError(err.error || 'Failed to update board');
    } finally {
      setSaving(false);
    }
  };

  return (
    <div style={styles.modal(isMobile)} onClick={onClose}>
      <div style={styles.modalContent(isMobile)} onClick={e => e.stopPropagation()}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
          <h2 style={{ color: '#f1f5f9', fontSize: '1.1rem', margin: 0 }}>⚙️ Board Settings</h2>
          <button style={styles.btnSmall} onClick={onClose}>✕</button>
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
          style={{ ...styles.input, minHeight: '60px', resize: 'vertical' }}
          value={description}
          onChange={e => setDescription(e.target.value)}
          disabled={!canEdit}
        />

        <label style={{ color: '#94a3b8', fontSize: '0.8rem', display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '16px', cursor: canEdit ? 'pointer' : 'default' }}>
          <input
            type="checkbox"
            checked={isPublic}
            onChange={e => setIsPublic(e.target.checked)}
            disabled={!canEdit}
          />
          Public (listed in board directory)
        </label>

        <div style={{ color: '#64748b', fontSize: '0.75rem', marginBottom: '16px' }}>
          <div>Board ID: <code style={{ color: '#94a3b8' }}>{board.id}</code></div>
          <div>Created: {new Date(board.created_at).toLocaleString()}</div>
        </div>

        {canEdit && (
          <button
            style={styles.btn('primary', isMobile)}
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? 'Saving...' : 'Save Changes'}
          </button>
        )}
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
          <button style={styles.btnSmall} onClick={onClose}>✕</button>
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
            <button style={styles.btnSmall} onClick={() => setCreatedSecret(null)}>Dismiss</button>
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

function LiveIndicator({ status }) {
  const color = status === 'connected' ? '#22c55e' : status === 'disconnected' ? '#ef4444' : '#eab308';
  const title = status === 'connected' ? 'Live — real-time sync active' : status === 'disconnected' ? 'Reconnecting…' : 'Connecting…';
  return (
    <span title={title} style={{
      display: 'inline-flex', alignItems: 'center', gap: '4px',
      fontSize: '0.65rem', color: status === 'connected' ? '#64748b' : color, fontWeight: 500,
      padding: '2px 6px', borderRadius: '10px', cursor: 'default',
    }}>
      <span style={{
        width: '6px', height: '6px', borderRadius: '50%', background: color,
        ...(status === 'connected' ? { animation: 'pulse 2s ease-in-out infinite' } : {}),
      }} />
      {status !== 'connected' && title}
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

  return (
    <>
      <div style={{ position: 'fixed', inset: 0, zIndex: 299 }} onClick={onClose} />
      <div style={{
        position: 'absolute', top: '100%', right: 0, marginTop: '6px', zIndex: 300,
        background: '#1e293b', border: '1px solid #334155', borderRadius: '8px',
        padding: '12px', width: '320px', maxWidth: '90vw',
        boxShadow: '0 8px 24px rgba(0,0,0,0.4)',
      }}>
        <div style={{ fontSize: '0.75rem', fontWeight: 600, color: '#94a3b8', marginBottom: '10px', textTransform: 'uppercase', letterSpacing: '0.04em' }}>
          Share Board
        </div>

        {/* View URL */}
        <div style={{ marginBottom: canEdit ? '10px' : 0 }}>
          <div style={{ fontSize: '0.7rem', color: '#64748b', marginBottom: '4px' }}>👁️ Read-only link — anyone with this can view</div>
          <div style={{ display: 'flex', gap: '6px', alignItems: 'center' }}>
            <input readOnly value={viewUrl} style={{
              flex: 1, background: '#0f172a', color: '#e2e8f0', border: '1px solid #334155',
              borderRadius: '4px', padding: '5px 8px', fontSize: '0.75rem', outline: 'none',
            }} onClick={e => e.target.select()} />
            <button onClick={() => copy(viewUrl, 'view')} style={{
              background: copied === 'view' ? '#22c55e22' : '#334155', color: copied === 'view' ? '#22c55e' : '#e2e8f0',
              border: 'none', borderRadius: '4px', padding: '5px 8px', cursor: 'pointer', fontSize: '0.75rem', whiteSpace: 'nowrap',
            }}>{copied === 'view' ? '✓ Copied' : 'Copy'}</button>
          </div>
        </div>

        {/* Manage URL */}
        {canEdit && manageUrl && (
          <div>
            <div style={{ fontSize: '0.7rem', color: '#64748b', marginBottom: '4px' }}>✏️ Edit link — full access (keep private!)</div>
            <div style={{ display: 'flex', gap: '6px', alignItems: 'center' }}>
              <input readOnly value={manageUrl} style={{
                flex: 1, background: '#0f172a', color: '#e2e8f0', border: '1px solid #334155',
                borderRadius: '4px', padding: '5px 8px', fontSize: '0.75rem', outline: 'none',
              }} onClick={e => e.target.select()} />
              <button onClick={() => copy(manageUrl, 'manage')} style={{
                background: copied === 'manage' ? '#22c55e22' : '#334155', color: copied === 'manage' ? '#22c55e' : '#e2e8f0',
                border: 'none', borderRadius: '4px', padding: '5px 8px', cursor: 'pointer', fontSize: '0.75rem', whiteSpace: 'nowrap',
              }}>{copied === 'manage' ? '✓ Copied' : 'Copy'}</button>
            </div>
          </div>
        )}

        {/* Hint for view-only users */}
        {!canEdit && (
          <div style={{ fontSize: '0.7rem', color: '#475569', marginTop: '8px', lineHeight: 1.4 }}>
            Need edit access? Open the board using the manage link (contains <code style={{ color: '#94a3b8' }}>?key=...</code>).
          </div>
        )}
      </div>
    </>
  );
}

// ---- Access Mode Indicator + Share ----
function AccessIndicator({ boardId, canEdit, isMobile }) {
  const [showShare, setShowShare] = useState(false);
  return (
    <div style={{ position: 'relative', display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
      <span style={{
        fontSize: '0.7rem', fontWeight: 600,
        padding: '3px 8px', borderRadius: '12px 0 0 12px',
        background: canEdit ? '#22c55e15' : '#64748b15',
        color: canEdit ? '#22c55e' : '#94a3b8',
        border: `1px solid ${canEdit ? '#22c55e33' : '#64748b33'}`,
        borderRight: 'none', whiteSpace: 'nowrap',
      }}>
        {canEdit ? (isMobile ? '✏️' : '✏️ Full Access') : (isMobile ? '👁️' : '👁️ View Only')}
      </span>
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

function BoardView({ board, canEdit, onRefresh, onBoardRefresh, isMobile }) {
  const [tasks, setTasks] = useState([]);
  const [showCreate, setShowCreate] = useState(false);
  const [search, setSearch] = useState('');
  const [searchResults, setSearchResults] = useState(null);
  const [selectedTask, setSelectedTask] = useState(null);
  const [sseStatus, setSseStatus] = useState('connecting');
  const [addingColumn, setAddingColumn] = useState(false);
  const [newColumnName, setNewColumnName] = useState('');
  const [showWebhooks, setShowWebhooks] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [filterPriority, setFilterPriority] = useState('');
  const [filterLabel, setFilterLabel] = useState('');
  const [filterAssignee, setFilterAssignee] = useState('');
  const [showFilters, setShowFilters] = useState(false);
  const [collapsedColumns, setCollapsedColumns] = useState({});
  const toggleColumnCollapse = useCallback((colId) => {
    setCollapsedColumns(prev => ({ ...prev, [colId]: !prev[colId] }));
  }, []);

  const loadTasks = useCallback(async () => {
    try {
      const { data } = await api.listTasks(board.id);
      setTasks(data.tasks || data || []);
    } catch (err) {
      console.error('Failed to load tasks:', err);
    }
  }, [board.id]);

  useEffect(() => { loadTasks(); }, [loadTasks]);

  // SSE: subscribe to real-time board events (debounced refresh)
  useEffect(() => {
    setSseStatus('connecting');
    let debounceTimer = null;
    const debouncedRefresh = () => {
      if (debounceTimer) clearTimeout(debounceTimer);
      debounceTimer = setTimeout(() => loadTasks(), 300);
    };
    const sub = api.subscribeToBoardEvents(
      board.id,
      (evt) => {
        // On any task event, debounce-refresh the task list
        if (evt.event !== 'warning') {
          debouncedRefresh();
        }
      },
      (status) => setSseStatus(status),
    );
    return () => {
      if (debounceTimer) clearTimeout(debounceTimer);
      sub.close();
    };
  }, [board.id, loadTasks]);

  const doSearch = async () => {
    if (!search.trim()) { setSearchResults(null); return; }
    try {
      const { data } = await api.searchTasks(board.id, search.trim());
      setSearchResults(data.tasks || []);
    } catch (err) {
      console.error('Search failed:', err);
    }
  };

  const columns = board.columns || [];
  const baseTasks = searchResults !== null ? searchResults : tasks;

  // Collect unique labels and assignees for filter dropdowns
  const allLabels = [...new Set(baseTasks.flatMap(t => (Array.isArray(t.labels) ? t.labels : (t.labels || '').split(',').map(l => l.trim())).filter(Boolean)))].sort();
  const allAssignees = [...new Set(baseTasks.map(t => t.assigned_to || t.claimed_by).filter(Boolean))].sort();

  // Apply filters
  const displayTasks = baseTasks.filter(t => {
    if (filterPriority && String(t.priority) !== filterPriority) return false;
    if (filterLabel && !(Array.isArray(t.labels) ? t.labels.join(',') : (t.labels || '')).toLowerCase().includes(filterLabel.toLowerCase())) return false;
    if (filterAssignee && t.assigned_to !== filterAssignee && t.claimed_by !== filterAssignee) return false;
    return true;
  });
  const hasActiveFilters = filterPriority || filterLabel || filterAssignee;
  const archived = !!board.archived_at;

  return (
    <div style={styles.boardContent}>
      <div style={styles.boardHeader(isMobile)}>
        <div style={{ minWidth: 0 }}>
          <span style={styles.boardTitle(isMobile)}>{board.name}</span>
          {archived && <span style={{ ...styles.archivedBadge, marginLeft: '10px' }}>ARCHIVED</span>}
          {board.description && (
            <p style={{ fontSize: '0.8rem', color: '#64748b', marginTop: '4px' }}>{board.description}</p>
          )}
        </div>
        <div style={{ display: 'flex', gap: isMobile ? '6px' : '8px', alignItems: 'center', flexShrink: 0, flexWrap: 'wrap' }}>
          {canEdit && !archived && (
            <button style={{ ...styles.btn('primary', isMobile), order: isMobile ? -1 : 0 }} onClick={() => setShowCreate(true)}>+ Task</button>
          )}
          <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
            <LiveIndicator status={sseStatus} />
            <AccessIndicator boardId={board.id} canEdit={canEdit} isMobile={isMobile} />
            <button style={styles.btnSmall} onClick={() => setShowSettings(true)} title="Board Settings">⚙️</button>
            {canEdit && !archived && (
              <button style={styles.btnSmall} onClick={() => setShowWebhooks(true)} title="Webhooks">⚡</button>
            )}
          </div>
        </div>
      </div>

      <div style={styles.searchBar(isMobile)}>
        <div style={{ position: 'relative', flex: 1, display: 'flex', alignItems: 'center' }}>
          <input
            style={{ ...styles.input, marginBottom: 0, width: '100%', paddingRight: search ? '28px' : undefined }}
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
        <button style={styles.btnSmall} onClick={doSearch}>Search</button>
        <button style={{ ...styles.btnSmall, background: hasActiveFilters ? '#3b82f633' : '#1e293b', color: hasActiveFilters ? '#3b82f6' : '#94a3b8', border: `1px solid ${hasActiveFilters ? '#3b82f644' : '#334155'}` }} onClick={() => setShowFilters(f => !f)}>
          {showFilters ? '▲' : '▼'} Filter{hasActiveFilters ? ' ●' : ''}
        </button>
      </div>
      {showFilters && (
        <div style={{ display: 'flex', gap: '8px', padding: '8px 16px', flexWrap: 'wrap', alignItems: 'center', background: '#1a2332', borderBottom: '1px solid #1e293b' }}>
          <select style={{ ...styles.select, marginBottom: 0, flex: 'none', minWidth: '120px' }} value={filterPriority} onChange={e => setFilterPriority(e.target.value)}>
            <option value="">Any Priority</option>
            <option value="1">🔴 Critical</option>
            <option value="2">🟠 High</option>
            <option value="3">🟡 Medium</option>
            <option value="4">🟢 Low</option>
          </select>
          <select style={{ ...styles.select, marginBottom: 0, flex: 'none', minWidth: '120px' }} value={filterLabel} onChange={e => setFilterLabel(e.target.value)}>
            <option value="">Any Label</option>
            {allLabels.map(l => <option key={l} value={l}>{l}</option>)}
          </select>
          <select style={{ ...styles.select, marginBottom: 0, flex: 'none', minWidth: '120px' }} value={filterAssignee} onChange={e => setFilterAssignee(e.target.value)}>
            <option value="">Any Assignee</option>
            {allAssignees.map(a => <option key={a} value={a}>{a}</option>)}
          </select>
          {hasActiveFilters && (
            <button style={styles.btnSmall} onClick={() => { setFilterPriority(''); setFilterLabel(''); setFilterAssignee(''); }}>Clear Filters</button>
          )}
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
            onRefresh={loadTasks}
            onBoardRefresh={onBoardRefresh}
            archived={archived}
            onClickTask={setSelectedTask}
            isMobile={isMobile}
            allColumns={columns}
            collapsed={collapsedColumns[col.id]}
            onToggleCollapse={() => toggleColumnCollapse(col.id)}
          />
        ))}
        {canEdit && !archived && (
          addingColumn ? (
            <div style={{ ...styles.column(false, isMobile), minWidth: isMobile ? undefined : '200px', maxWidth: isMobile ? undefined : '200px', justifyContent: 'flex-start' }}>
              <input
                autoFocus
                style={{ background: '#1e293b', color: '#e2e8f0', border: '1px solid #3b82f6', borderRadius: '4px', padding: '6px 8px', fontSize: '0.85rem', width: '100%' }}
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

      {showCreate && (
        <CreateTaskModal
          boardId={board.id}
          columns={columns}
          onClose={() => setShowCreate(false)}
          onCreated={loadTasks}
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
          onRefresh={loadTasks}
          isMobile={isMobile}
          allColumns={columns}
          allLabels={allLabels}
          allAssignees={allAssignees}
        />
      )}

      {showSettings && (
        <BoardSettingsModal
          board={board}
          canEdit={canEdit}
          onClose={() => setShowSettings(false)}
          onRefresh={onBoardRefresh}
          isMobile={isMobile}
        />
      )}

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

function App() {
  const { isMobile, isCompact } = useBreakpoint();
  const collapseSidebar = isCompact; // collapse sidebar on mobile + tablet
  const [boards, setBoards] = useState([]);
  const [selectedBoardId, setSelectedBoardId] = useState(null);
  const [boardDetail, setBoardDetail] = useState(null);
  const [showCreateBoard, setShowCreateBoard] = useState(false);
  const [showArchived, setShowArchived] = useState(false);
  const [loadError, setLoadError] = useState(null);
  const [sidebarOpen, setSidebarOpen] = useState(false);

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

  const loadBoards = useCallback(async () => {
    try {
      const { data } = await api.listBoards(showArchived);
      setBoards(data.boards || data || []);
    } catch (err) {
      console.error('Failed to load boards:', err);
    }
  }, [showArchived]);

  useEffect(() => { loadBoards(); }, [loadBoards]);

  const loadBoardDetail = useCallback(async (boardId) => {
    const id = boardId || selectedBoardId;
    if (!id) { setBoardDetail(null); setLoadError(null); return; }
    setLoadError(null);
    try {
      const { data } = await api.getBoard(id);
      setBoardDetail(data);
    } catch (err) {
      console.error('Failed to load board:', err);
      setLoadError(err.status === 404 ? 'Board not found.' : 'Failed to load board.');
      setBoardDetail(null);
    }
  }, [selectedBoardId]);

  useEffect(() => {
    if (!selectedBoardId) { setBoardDetail(null); setLoadError(null); return; }
    loadBoardDetail(selectedBoardId);
  }, [selectedBoardId, loadBoardDetail]);

  const canEdit = selectedBoardId ? api.hasBoardKey(selectedBoardId) : false;

  const handleBoardCreated = (newBoardId) => {
    loadBoards();
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

  return (
    <div style={styles.app}>
      <div style={styles.header(isMobile)}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
          {collapseSidebar && (
            <button style={styles.menuBtn} onClick={() => setSidebarOpen(o => !o)}>☰</button>
          )}
          <div style={styles.logo} onClick={() => { setSelectedBoardId(null); setBoardDetail(null); }}>
            <img src="/logo.svg" alt="" style={styles.logoImg} />
            Kanban
          </div>
        </div>
        <div style={styles.headerRight}>
          {selectedBoardId && canEdit && (
            <IdentityBadge isMobile={isMobile} />
          )}
          {selectedBoardId && (
            <AccessIndicator boardId={selectedBoardId} canEdit={canEdit} isMobile={isMobile} />
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
            <span>Public Boards</span>
            <button style={styles.btnSmall} onClick={() => { setShowCreateBoard(true); setSidebarOpen(false); }}>+ New</button>
          </div>
          {boards.map(b => (
            <div
              key={b.id}
              style={styles.boardItem(selectedBoardId === b.id)}
              onClick={() => handleSelectBoard(b.id)}
            >
              <span>{b.name}</span>
              {b.archived_at && <span style={styles.archivedBadge}>📦</span>}
            </div>
          ))}
          {boards.length === 0 && (
            <div style={{ ...styles.empty, padding: '20px 16px', fontSize: '0.8rem' }}>
              No public boards yet.
            </div>
          )}

          <div style={{ borderTop: '1px solid #334155', marginTop: 'auto', padding: '12px' }}>
            <DirectBoardInput onOpen={handleOpenDirect} />
            <label style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '0.75rem', color: '#64748b', cursor: 'pointer', padding: '8px 4px 0' }}>
              <input
                type="checkbox"
                checked={showArchived}
                onChange={e => setShowArchived(e.target.checked)}
                style={{ accentColor: '#6366f1' }}
              />
              Show archived boards
            </label>
          </div>
        </div>

        {boardDetail ? (
          <BoardView board={boardDetail} canEdit={canEdit} onRefresh={() => loadBoardDetail()} onBoardRefresh={() => loadBoardDetail()} isMobile={isMobile} />
        ) : loadError ? (
          <div style={{ ...styles.boardContent, ...styles.empty, justifyContent: 'center', display: 'flex', alignItems: 'center' }}>
            <div>
              <p style={{ fontSize: '1.1rem', marginBottom: '8px', color: '#ef4444' }}>{loadError}</p>
              <p style={{ fontSize: '0.85rem' }}>Check the board ID and try again.</p>
            </div>
          </div>
        ) : (
          <div style={{ ...styles.boardContent, ...styles.empty, justifyContent: 'center', display: 'flex', alignItems: 'center', padding: '20px' }}>
            <div>
              <p style={{ fontSize: '1.5rem', marginBottom: '8px', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}><img src="/logo.svg" alt="" style={{ width: '28px', height: '28px' }} /> Kanban</p>
              <p style={{ color: '#94a3b8', marginBottom: '4px' }}>Humans Not Required</p>
              <p style={{ fontSize: '0.85rem', maxWidth: '400px', lineHeight: '1.5' }}>
                {collapseSidebar ? 'Tap ☰ to browse boards, or create a new one.' : 'Select a public board, open one by ID, or create a new one.'}
                <br />
                <span style={{ color: '#64748b', fontSize: '0.8rem' }}>
                  No signup required.
                </span>
              </p>
              {isMobile && (
                <button style={{ ...styles.btn('primary', true), marginTop: '12px' }} onClick={() => setShowCreateBoard(true)}>
                  + New Board
                </button>
              )}
            </div>
          </div>
        )}
      </div>

      {showCreateBoard && (
        <CreateBoardModal
          onClose={() => setShowCreateBoard(false)}
          onCreated={handleBoardCreated}
          isMobile={isMobile}
        />
      )}
    </div>
  );
}

export default App;
