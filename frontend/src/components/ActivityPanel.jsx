import { useState, useEffect, useCallback } from "react";
import * as api from "../api";
import styles from "../styles";
import { formatEventDescription, formatTimeAgo, eventIcon, setLastVisit } from "../utils";
import { useEscapeKey } from "../hooks";

function ActivityPanel({ boardId, onClose, isMobile, onOpenTask }) {
  useEscapeKey(onClose);
  const [tab, setTab] = useState('mine'); // 'mine' | 'all'
  const [activity, setActivity] = useState([]);
  const [myTasks, setMyTasks] = useState([]);
  const [loading, setLoading] = useState(true);
  const [sortMode, setSortMode] = useState('priority'); // 'priority' | 'column'
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

  // Load my items (assigned tasks)
  useEffect(() => {
    if (tab !== 'mine') return;
    if (!displayName) { setLoading(false); return; }
    setLoading(true);
    (async () => {
      try {
        const tasksRes = await api.listTasks(boardId, `assigned=${encodeURIComponent(displayName)}`);
        setMyTasks((tasksRes.data || []).filter(t => !t.archived_at));
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
          <button style={tabStyle(tab === 'all')} onClick={() => setTab('all')}>
            📋 All Activity
          </button>
        </div>

        {loading ? (
          <div style={{ color: '#64748b', textAlign: 'center', padding: '20px' }}>Loading...</div>
        ) : tab === 'mine' ? renderMyItemsTab() : renderActivityList()}
      </div>
    </div>
  );
}


export default ActivityPanel;
