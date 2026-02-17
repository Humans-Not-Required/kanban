import { useState, useEffect, useRef } from 'react';
import * as api from '../api';
import styles from '../styles';
import { TASKS_PER_PAGE } from '../utils';
import { useEscapeKey } from '../hooks';
import TaskCard from './TaskCard';

export function FullScreenColumnView({ column, tasks, boardId, canEdit, onRefresh, onClose, onClickTask, archived }) {
  useEscapeKey(onClose);
  const colTasks = tasks.filter(t => t.column_id === column.id)
    .sort((a, b) => (b.priority || 0) - (a.priority || 0) || (a.title || '').localeCompare(b.title || ''));

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

export default function Column({ column, tasks, boardId, canEdit, onRefresh, onBoardRefresh, archived, onClickTask, isMobile, allColumns, collapsed: externalCollapsed, onToggleCollapse, tasksLoaded, onFullScreen }) {
  const [dragOver, setDragOver] = useState(false);
  const colTaskCount = tasks.filter(t => t.column_id === column.id).length;
  const [internalCollapsed, setInternalCollapsed] = useState(false);
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
