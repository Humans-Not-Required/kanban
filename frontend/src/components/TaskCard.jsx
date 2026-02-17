import { useState } from 'react';
import * as api from '../api';
import styles from '../styles';
import { parseUTC, priorityColor, priorityLabel } from '../utils';

export function MoveTaskDropdown({ boardId, task, columns, onMoved, onCancel }) {
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

export default function TaskCard({ task, boardId, canEdit, onRefresh, archived, onClickTask, isMobile }) {
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
