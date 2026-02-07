import { useState, useEffect, useCallback } from 'react';
import * as api from './api';

// ---- Styles ----
const styles = {
  app: { minHeight: '100vh', display: 'flex', flexDirection: 'column' },
  header: {
    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
    padding: '12px 20px', background: '#1e293b', borderBottom: '1px solid #334155',
  },
  logo: { fontSize: '1.2rem', fontWeight: 700, color: '#f1f5f9' },
  headerRight: { display: 'flex', alignItems: 'center', gap: '12px', fontSize: '0.85rem' },
  rateInfo: { color: '#94a3b8' },
  keyInput: {
    background: '#0f172a', border: '1px solid #334155', color: '#e2e8f0',
    padding: '4px 8px', borderRadius: '4px', fontSize: '0.8rem', width: '200px',
  },
  main: { flex: 1, display: 'flex', overflow: 'hidden' },
  sidebar: {
    width: '240px', minWidth: '240px', background: '#1e293b',
    borderRight: '1px solid #334155', display: 'flex', flexDirection: 'column',
    overflow: 'auto',
  },
  sidebarHeader: {
    padding: '12px 16px', fontSize: '0.75rem', fontWeight: 600, color: '#94a3b8',
    textTransform: 'uppercase', letterSpacing: '0.05em',
    display: 'flex', justifyContent: 'space-between', alignItems: 'center',
  },
  boardItem: (active) => ({
    padding: '8px 16px', cursor: 'pointer', fontSize: '0.9rem',
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
  boardHeader: {
    padding: '16px 20px', display: 'flex', alignItems: 'center', justifyContent: 'space-between',
    borderBottom: '1px solid #1e293b',
  },
  boardTitle: { fontSize: '1.3rem', fontWeight: 700, color: '#f1f5f9' },
  columnsContainer: {
    flex: 1, display: 'flex', gap: '16px', padding: '16px 20px',
    overflowX: 'auto', alignItems: 'flex-start',
  },
  column: (isDragOver) => ({
    minWidth: '280px', maxWidth: '320px', flex: '0 0 280px',
    background: isDragOver ? '#1e293b' : '#1a2332', borderRadius: '8px',
    border: isDragOver ? '2px dashed #6366f1' : '1px solid #334155',
    display: 'flex', flexDirection: 'column', maxHeight: 'calc(100vh - 160px)',
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
  taskList: { flex: 1, overflow: 'auto', padding: '8px' },
  card: (isDragging, priority) => ({
    background: isDragging ? '#334155' : '#0f172a',
    border: `1px solid ${priorityColor(priority)}33`,
    borderLeft: `3px solid ${priorityColor(priority)}`,
    borderRadius: '6px', padding: '10px 12px', marginBottom: '8px',
    cursor: 'grab', opacity: isDragging ? 0.5 : 1,
    transition: 'all 0.15s ease',
  }),
  cardTitle: { fontSize: '0.88rem', fontWeight: 500, color: '#e2e8f0', marginBottom: '4px' },
  cardMeta: { display: 'flex', gap: '8px', fontSize: '0.73rem', color: '#64748b', flexWrap: 'wrap' },
  label: (color) => ({
    background: color || '#6366f133', color: color ? '#fff' : '#a5b4fc',
    padding: '1px 6px', borderRadius: '3px', fontSize: '0.68rem',
  }),
  btn: (variant = 'primary') => ({
    background: variant === 'primary' ? '#6366f1' : variant === 'danger' ? '#ef4444' : '#334155',
    color: '#fff', border: 'none', padding: '6px 12px', borderRadius: '4px',
    cursor: 'pointer', fontSize: '0.8rem', fontWeight: 500,
  }),
  btnSmall: {
    background: 'transparent', border: '1px solid #334155', color: '#94a3b8',
    padding: '3px 8px', borderRadius: '4px', cursor: 'pointer', fontSize: '0.75rem',
  },
  modal: {
    position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.6)',
    display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 100,
  },
  modalContent: {
    background: '#1e293b', border: '1px solid #334155', borderRadius: '8px',
    padding: '24px', width: '480px', maxWidth: '90vw', maxHeight: '85vh', overflow: 'auto',
  },
  input: {
    width: '100%', background: '#0f172a', border: '1px solid #334155', color: '#e2e8f0',
    padding: '8px 10px', borderRadius: '4px', fontSize: '0.9rem', marginBottom: '10px',
  },
  textarea: {
    width: '100%', background: '#0f172a', border: '1px solid #334155', color: '#e2e8f0',
    padding: '8px 10px', borderRadius: '4px', fontSize: '0.85rem', minHeight: '80px',
    resize: 'vertical', marginBottom: '10px', fontFamily: 'inherit',
  },
  select: {
    background: '#0f172a', border: '1px solid #334155', color: '#e2e8f0',
    padding: '6px 8px', borderRadius: '4px', fontSize: '0.85rem', marginBottom: '10px',
  },
  empty: {
    textAlign: 'center', color: '#475569', padding: '40px 20px', fontSize: '0.9rem',
  },
  searchBar: {
    display: 'flex', gap: '8px', padding: '0 20px', paddingBottom: '0',
  },
};

function priorityColor(p) {
  if (p === 'critical') return '#ef4444';
  if (p === 'high') return '#f97316';
  if (p === 'medium') return '#eab308';
  if (p === 'low') return '#22c55e';
  return '#64748b';
}

// ---- Components ----

function TaskCard({ task, boardId, onRefresh, archived }) {
  const [dragging, setDragging] = useState(false);
  return (
    <div
      style={styles.card(dragging, task.priority)}
      draggable={!archived}
      onDragStart={(e) => { setDragging(true); e.dataTransfer.setData('taskId', task.id); }}
      onDragEnd={() => setDragging(false)}
    >
      <div style={styles.cardTitle}>{task.title}</div>
      <div style={styles.cardMeta}>
        <span style={{ color: priorityColor(task.priority) }}>{task.priority}</span>
        {task.assigned_to && <span>→ {task.assigned_to}</span>}
        {task.claimed_by && <span>🔒 {task.claimed_by}</span>}
        {task.due_at && <span>📅 {new Date(task.due_at).toLocaleDateString()}</span>}
        {task.completed_at && <span>✅</span>}
      </div>
      {task.labels && task.labels.length > 0 && (
        <div style={{ display: 'flex', gap: '4px', marginTop: '6px', flexWrap: 'wrap' }}>
          {task.labels.map((l, i) => <span key={i} style={styles.label()}>{l}</span>)}
        </div>
      )}
    </div>
  );
}

function Column({ column, tasks, boardId, onRefresh, archived }) {
  const [dragOver, setDragOver] = useState(false);
  const colTasks = tasks.filter(t => t.column_id === column.id)
    .sort((a, b) => (a.position ?? 999) - (b.position ?? 999));

  const handleDrop = async (e) => {
    e.preventDefault();
    setDragOver(false);
    const taskId = e.dataTransfer.getData('taskId');
    if (!taskId || archived) return;
    try {
      await api.moveTask(boardId, taskId, column.id);
      onRefresh();
    } catch (err) {
      if (err.code === 'WIP_LIMIT_EXCEEDED') {
        alert(`WIP limit reached for "${column.name}" (max ${column.wip_limit})`);
      }
    }
  };

  const wipInfo = column.wip_limit
    ? `${colTasks.length}/${column.wip_limit}`
    : `${colTasks.length}`;

  return (
    <div
      style={styles.column(dragOver)}
      onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
      onDragLeave={() => setDragOver(false)}
      onDrop={handleDrop}
    >
      <div style={styles.columnHeader}>
        <span>{column.name}</span>
        <span style={styles.taskCount}>{wipInfo}</span>
      </div>
      <div style={styles.taskList}>
        {colTasks.length === 0 && (
          <div style={{ ...styles.empty, padding: '20px 10px', fontSize: '0.8rem' }}>
            Drop tasks here
          </div>
        )}
        {colTasks.map(t => (
          <TaskCard key={t.id} task={t} boardId={boardId} onRefresh={onRefresh} archived={archived} />
        ))}
      </div>
    </div>
  );
}

function CreateTaskModal({ boardId, columns, onClose, onCreated }) {
  const [title, setTitle] = useState('');
  const [desc, setDesc] = useState('');
  const [priority, setPriority] = useState('medium');
  const [columnId, setColumnId] = useState(columns[0]?.id || '');
  const [labels, setLabels] = useState('');
  const [assignedTo, setAssignedTo] = useState('');
  const [loading, setLoading] = useState(false);

  const submit = async (e) => {
    e.preventDefault();
    if (!title.trim()) return;
    setLoading(true);
    try {
      await api.createTask(boardId, {
        title: title.trim(),
        description: desc.trim() || undefined,
        priority,
        column_id: columnId,
        labels: labels.trim() ? labels.split(',').map(l => l.trim()).filter(Boolean) : [],
        assigned_to: assignedTo.trim() || undefined,
      });
      onCreated();
      onClose();
    } catch (err) {
      alert(err.error || 'Failed to create task');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={styles.modal} onClick={onClose}>
      <div style={styles.modalContent} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ marginBottom: '16px', color: '#f1f5f9' }}>New Task</h3>
        <form onSubmit={submit}>
          <input style={styles.input} placeholder="Title" value={title} onChange={e => setTitle(e.target.value)} autoFocus />
          <textarea style={styles.textarea} placeholder="Description (optional)" value={desc} onChange={e => setDesc(e.target.value)} />
          <div style={{ display: 'flex', gap: '10px', marginBottom: '10px' }}>
            <select style={styles.select} value={priority} onChange={e => setPriority(e.target.value)}>
              <option value="critical">Critical</option>
              <option value="high">High</option>
              <option value="medium">Medium</option>
              <option value="low">Low</option>
            </select>
            <select style={styles.select} value={columnId} onChange={e => setColumnId(e.target.value)}>
              {columns.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
            </select>
          </div>
          <input style={styles.input} placeholder="Labels (comma-separated)" value={labels} onChange={e => setLabels(e.target.value)} />
          <input style={styles.input} placeholder="Assigned to (optional)" value={assignedTo} onChange={e => setAssignedTo(e.target.value)} />
          <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
            <button type="button" style={styles.btn('secondary')} onClick={onClose}>Cancel</button>
            <button type="submit" style={styles.btn('primary')} disabled={loading || !title.trim()}>
              {loading ? 'Creating...' : 'Create Task'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

function CreateBoardModal({ onClose, onCreated }) {
  const [name, setName] = useState('');
  const [desc, setDesc] = useState('');
  const [columns, setColumns] = useState('To Do, In Progress, Done');
  const [loading, setLoading] = useState(false);

  const submit = async (e) => {
    e.preventDefault();
    if (!name.trim()) return;
    setLoading(true);
    try {
      const cols = columns.split(',').map(c => c.trim()).filter(Boolean);
      await api.createBoard({
        name: name.trim(),
        description: desc.trim() || undefined,
        columns: cols.map((n, i) => ({
          name: n,
          position: i,
          is_done_column: i === cols.length - 1,
        })),
      });
      onCreated();
      onClose();
    } catch (err) {
      alert(err.error || 'Failed to create board');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={styles.modal} onClick={onClose}>
      <div style={styles.modalContent} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ marginBottom: '16px', color: '#f1f5f9' }}>New Board</h3>
        <form onSubmit={submit}>
          <input style={styles.input} placeholder="Board Name" value={name} onChange={e => setName(e.target.value)} autoFocus />
          <textarea style={styles.textarea} placeholder="Description (optional)" value={desc} onChange={e => setDesc(e.target.value)} />
          <input style={styles.input} placeholder="Columns (comma-separated)" value={columns} onChange={e => setColumns(e.target.value)} />
          <p style={{ fontSize: '0.73rem', color: '#64748b', marginBottom: '12px' }}>
            Last column is automatically marked as "done" column.
          </p>
          <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
            <button type="button" style={styles.btn('secondary')} onClick={onClose}>Cancel</button>
            <button type="submit" style={styles.btn('primary')} disabled={loading || !name.trim()}>
              {loading ? 'Creating...' : 'Create Board'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

function BoardView({ board, onRefresh }) {
  const [tasks, setTasks] = useState([]);
  const [showCreate, setShowCreate] = useState(false);
  const [search, setSearch] = useState('');
  const [searchResults, setSearchResults] = useState(null);

  const loadTasks = useCallback(async () => {
    try {
      const { data } = await api.listTasks(board.id);
      setTasks(data.tasks || data || []);
    } catch (err) {
      console.error('Failed to load tasks:', err);
    }
  }, [board.id]);

  useEffect(() => { loadTasks(); }, [loadTasks]);

  // SSE for real-time updates
  useEffect(() => {
    const key = api.getKey();
    if (!key) return;
    const url = `/api/v1/boards/${board.id}/events/stream`;
    const es = new EventSource(url);
    // EventSource doesn't support custom headers, so SSE won't work without cookie/query auth.
    // For now, we'll poll on task changes via the create/move callbacks.
    // TODO: Add query-param auth support to backend for SSE
    es.onerror = () => es.close();
    return () => es.close();
  }, [board.id]);

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
  const displayTasks = searchResults !== null ? searchResults : tasks;
  const archived = !!board.archived_at;

  return (
    <div style={styles.boardContent}>
      <div style={styles.boardHeader}>
        <div>
          <span style={styles.boardTitle}>{board.name}</span>
          {archived && <span style={{ ...styles.archivedBadge, marginLeft: '10px' }}>ARCHIVED</span>}
          {board.description && (
            <p style={{ fontSize: '0.8rem', color: '#64748b', marginTop: '4px' }}>{board.description}</p>
          )}
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          {!archived && (
            <button style={styles.btn('primary')} onClick={() => setShowCreate(true)}>+ New Task</button>
          )}
        </div>
      </div>

      <div style={styles.searchBar}>
        <input
          style={{ ...styles.input, marginBottom: 0, flex: 1 }}
          placeholder="Search tasks..."
          value={search}
          onChange={e => setSearch(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && doSearch()}
        />
        <button style={styles.btnSmall} onClick={doSearch}>Search</button>
        {searchResults !== null && (
          <button style={styles.btnSmall} onClick={() => { setSearch(''); setSearchResults(null); }}>Clear</button>
        )}
      </div>

      <div style={styles.columnsContainer}>
        {columns.sort((a, b) => a.position - b.position).map(col => (
          <Column
            key={col.id}
            column={col}
            tasks={displayTasks}
            boardId={board.id}
            onRefresh={loadTasks}
            archived={archived}
          />
        ))}
        {columns.length === 0 && (
          <div style={styles.empty}>No columns. Add columns via the API.</div>
        )}
      </div>

      {showCreate && (
        <CreateTaskModal
          boardId={board.id}
          columns={columns}
          onClose={() => setShowCreate(false)}
          onCreated={loadTasks}
        />
      )}
    </div>
  );
}

function App() {
  const [apiKey, setApiKey] = useState(api.getKey());
  const [boards, setBoards] = useState([]);
  const [selectedBoard, setSelectedBoard] = useState(null);
  const [boardDetail, setBoardDetail] = useState(null);
  const [rateLimit, setRateLimit] = useState(null);
  const [showCreateBoard, setShowCreateBoard] = useState(false);
  const [showArchived, setShowArchived] = useState(false);

  const loadBoards = useCallback(async () => {
    try {
      const { data, rateLimit: rl } = await api.listBoards(showArchived);
      setBoards(data.boards || data || []);
      if (rl.remaining) setRateLimit(rl);
    } catch (err) {
      if (err.status === 401) setBoards([]);
      console.error('Failed to load boards:', err);
    }
  }, [showArchived]);

  useEffect(() => {
    if (apiKey) loadBoards();
  }, [apiKey, loadBoards]);

  useEffect(() => {
    if (!selectedBoard) { setBoardDetail(null); return; }
    (async () => {
      try {
        const { data, rateLimit: rl } = await api.getBoard(selectedBoard);
        setBoardDetail(data);
        if (rl.remaining) setRateLimit(rl);
      } catch (err) {
        console.error('Failed to load board:', err);
      }
    })();
  }, [selectedBoard]);

  const handleKeyChange = (e) => {
    const key = e.target.value;
    setApiKey(key);
    api.setKey(key);
  };

  if (!apiKey) {
    return (
      <div style={{ ...styles.app, alignItems: 'center', justifyContent: 'center' }}>
        <div style={{ textAlign: 'center', maxWidth: '400px' }}>
          <h1 style={{ fontSize: '2rem', marginBottom: '8px' }}>📋 Kanban</h1>
          <p style={{ color: '#94a3b8', marginBottom: '24px' }}>Humans Not Required</p>
          <input
            style={{ ...styles.input, textAlign: 'center', fontSize: '1rem' }}
            placeholder="Enter your API key"
            value={apiKey}
            onChange={handleKeyChange}
            autoFocus
          />
          <p style={{ color: '#475569', fontSize: '0.8rem', marginTop: '8px' }}>
            Run the backend to get your admin API key.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div style={styles.app}>
      <div style={styles.header}>
        <div style={styles.logo}>📋 Kanban</div>
        <div style={styles.headerRight}>
          {rateLimit && rateLimit.remaining && (
            <span style={styles.rateInfo}>
              {rateLimit.remaining}/{rateLimit.limit} req
            </span>
          )}
          <input
            style={styles.keyInput}
            type="password"
            value={apiKey}
            onChange={handleKeyChange}
            placeholder="API Key"
            title="API Key"
          />
        </div>
      </div>

      <div style={styles.main}>
        <div style={styles.sidebar}>
          <div style={styles.sidebarHeader}>
            <span>Boards</span>
            <button style={styles.btnSmall} onClick={() => setShowCreateBoard(true)}>+</button>
          </div>
          {boards.map(b => (
            <div
              key={b.id}
              style={styles.boardItem(selectedBoard === b.id)}
              onClick={() => setSelectedBoard(b.id)}
            >
              <span>{b.name}</span>
              {b.archived_at && <span style={styles.archivedBadge}>📦</span>}
            </div>
          ))}
          {boards.length === 0 && (
            <div style={{ ...styles.empty, padding: '20px 16px' }}>
              {apiKey ? 'No boards yet.' : 'Set API key above.'}
            </div>
          )}
          <div style={{ padding: '8px 16px', borderTop: '1px solid #334155', marginTop: 'auto' }}>
            <label style={{ fontSize: '0.75rem', color: '#64748b', cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={showArchived}
                onChange={e => setShowArchived(e.target.checked)}
                style={{ marginRight: '6px' }}
              />
              Show archived
            </label>
          </div>
        </div>

        {boardDetail ? (
          <BoardView board={boardDetail} onRefresh={() => setSelectedBoard(s => s)} />
        ) : (
          <div style={{ ...styles.boardContent, ...styles.empty, justifyContent: 'center', display: 'flex', alignItems: 'center' }}>
            <div>
              <p style={{ fontSize: '1.1rem', marginBottom: '8px' }}>Select a board or create one</p>
              <p style={{ fontSize: '0.85rem' }}>Agent-centric task coordination</p>
            </div>
          </div>
        )}
      </div>

      {showCreateBoard && (
        <CreateBoardModal
          onClose={() => setShowCreateBoard(false)}
          onCreated={() => { loadBoards(); setShowCreateBoard(false); }}
        />
      )}
    </div>
  );
}

export default App;
