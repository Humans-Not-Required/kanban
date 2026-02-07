import { useState, useEffect, useCallback } from 'react';
import * as api from './api';

// ---- Styles ----
const styles = {
  app: { minHeight: '100vh', display: 'flex', flexDirection: 'column' },
  header: {
    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
    padding: '12px 20px', background: '#1e293b', borderBottom: '1px solid #334155',
  },
  logo: { fontSize: '1.2rem', fontWeight: 700, color: '#f1f5f9', cursor: 'pointer' },
  headerRight: { display: 'flex', alignItems: 'center', gap: '12px', fontSize: '0.85rem' },
  modeBadge: (canEdit) => ({
    fontSize: '0.75rem', fontWeight: 600,
    padding: '3px 10px', borderRadius: '12px',
    background: canEdit ? '#22c55e22' : '#64748b22',
    color: canEdit ? '#22c55e' : '#94a3b8',
    border: `1px solid ${canEdit ? '#22c55e44' : '#64748b44'}`,
  }),
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
    display: 'flex', flexDirection: 'column', maxHeight: 'calc(100vh - 200px)',
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
    cursor: isDragging ? 'grabbing' : 'default',
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
  urlBox: {
    background: '#0f172a', border: '1px solid #334155', borderRadius: '4px',
    padding: '10px 14px', fontSize: '0.82rem', color: '#94a3b8',
    fontFamily: 'monospace', wordBreak: 'break-all', marginBottom: '10px',
    display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '10px',
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
    padding: '6px 10px', borderRadius: '4px', fontSize: '0.8rem', width: '240px',
  },
};

function priorityColor(p) {
  if (p === 'critical') return '#ef4444';
  if (p === 'high') return '#f97316';
  if (p === 'medium') return '#eab308';
  if (p === 'low') return '#22c55e';
  return '#64748b';
}

// ---- Copy to clipboard helper ----

function copyToClipboard(text) {
  navigator.clipboard.writeText(text).then(
    () => {},
    () => {
      // Fallback for older browsers
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

function TaskCard({ task, boardId, canEdit, onRefresh, archived }) {
  const [dragging, setDragging] = useState(false);
  const draggable = canEdit && !archived;

  return (
    <div
      style={{
        ...styles.card(dragging, task.priority),
        ...(draggable ? styles.cardDraggable : {}),
      }}
      draggable={draggable}
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

function Column({ column, tasks, boardId, canEdit, onRefresh, archived }) {
  const [dragOver, setDragOver] = useState(false);
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

  const wipInfo = column.wip_limit
    ? `${colTasks.length}/${column.wip_limit}`
    : `${colTasks.length}`;

  return (
    <div
      style={styles.column(dragOver && canEdit)}
      onDragOver={canEdit ? (e) => { e.preventDefault(); setDragOver(true); } : undefined}
      onDragLeave={canEdit ? () => setDragOver(false) : undefined}
      onDrop={canEdit ? handleDrop : undefined}
    >
      <div style={styles.columnHeader}>
        <span>{column.name}</span>
        <span style={styles.taskCount}>{wipInfo}</span>
      </div>
      <div style={styles.taskList}>
        {colTasks.length === 0 && (
          <div style={{ ...styles.empty, padding: '20px 10px', fontSize: '0.8rem' }}>
            {canEdit ? 'Drop tasks here' : 'No tasks'}
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
          />
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

  // After creation — show the manage URL
  if (result) {
    const origin = window.location.origin;
    const viewUrl = `${origin}/board/${result.board_id}`;
    const manageUrl = `${origin}/board/${result.board_id}?key=${result.manage_key}`;

    return (
      <div style={styles.modal} onClick={handleDone}>
        <div style={styles.modalContent} onClick={(e) => e.stopPropagation()}>
          <div style={styles.successBox}>
            <h3 style={{ color: '#22c55e', marginBottom: '8px' }}>✅ Board Created!</h3>
            <p style={{ color: '#94a3b8', fontSize: '0.85rem' }}>
              Save your management link — it's the only way to edit this board.
            </p>
          </div>

          <div style={{ marginBottom: '14px' }}>
            <div style={styles.urlLabel}>🔗 View Link (read-only, share freely)</div>
            <div style={styles.urlBox}>
              <span style={{ flex: 1 }}>{viewUrl}</span>
              <button
                style={styles.btnSmall}
                onClick={() => handleCopy(viewUrl, 'view')}
              >
                {copied === 'view' ? '✓ Copied' : 'Copy'}
              </button>
            </div>
          </div>

          <div style={{ marginBottom: '14px' }}>
            <div style={styles.urlLabel}>🔑 Manage Link (full access — keep private!)</div>
            <div style={{ ...styles.urlBox, borderColor: '#6366f155' }}>
              <span style={{ flex: 1, color: '#a5b4fc' }}>{manageUrl}</span>
              <button
                style={{ ...styles.btnSmall, borderColor: '#6366f1', color: '#a5b4fc' }}
                onClick={() => handleCopy(manageUrl, 'manage')}
              >
                {copied === 'manage' ? '✓ Copied' : 'Copy'}
              </button>
            </div>
          </div>

          <div style={{ marginBottom: '14px' }}>
            <div style={styles.urlLabel}>🤖 API Base (for programmatic access)</div>
            <div style={styles.urlBox}>
              <span style={{ flex: 1 }}>{origin}{result.api_base}</span>
              <button
                style={styles.btnSmall}
                onClick={() => handleCopy(`${origin}${result.api_base}`, 'api')}
              >
                {copied === 'api' ? '✓ Copied' : 'Copy'}
              </button>
            </div>
            <p style={{ fontSize: '0.73rem', color: '#64748b', marginTop: '4px' }}>
              Use <code style={{ color: '#94a3b8' }}>Authorization: Bearer {'{manage_key}'}</code> for write operations.
            </p>
          </div>

          <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
            <button style={styles.btn('primary')} onClick={handleDone}>
              Open Board →
            </button>
          </div>
        </div>
      </div>
    );
  }

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
          <label style={{ fontSize: '0.85rem', color: '#94a3b8', cursor: 'pointer', marginBottom: '12px', display: 'flex', alignItems: 'center', gap: '8px' }}>
            <input
              type="checkbox"
              checked={isPublic}
              onChange={e => setIsPublic(e.target.checked)}
            />
            Make board public (visible in board listing)
          </label>
          <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end', marginTop: '12px' }}>
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

function BoardView({ board, canEdit, onRefresh }) {
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
        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
          <span style={styles.modeBadge(canEdit)}>
            {canEdit ? '✏️ Edit Mode' : '👁️ View Only'}
          </span>
          {canEdit && !archived && (
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
            canEdit={canEdit}
            onRefresh={loadTasks}
            archived={archived}
          />
        ))}
        {columns.length === 0 && (
          <div style={styles.empty}>No columns yet.</div>
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

// ---- Open Board by ID (direct access) ----

function DirectBoardInput({ onOpen }) {
  const [boardId, setBoardId] = useState('');

  const submit = (e) => {
    e.preventDefault();
    const id = boardId.trim();
    if (!id) return;
    // Handle full URL or just board ID
    const match = id.match(/\/board\/([a-f0-9-]+)/i);
    onOpen(match ? match[1] : id);
    setBoardId('');
  };

  return (
    <form onSubmit={submit} style={{ display: 'flex', gap: '6px', padding: '8px 16px' }}>
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
  const [boards, setBoards] = useState([]);
  const [selectedBoardId, setSelectedBoardId] = useState(null);
  const [boardDetail, setBoardDetail] = useState(null);
  const [showCreateBoard, setShowCreateBoard] = useState(false);
  const [showArchived, setShowArchived] = useState(false);
  const [loadError, setLoadError] = useState(null);

  // On mount: check URL for ?key= and /board/{id} patterns
  useEffect(() => {
    const { boardId, key } = api.extractKeyFromUrl();
    if (boardId && key) {
      // Store the key for this board and clean URL
      api.setBoardKey(boardId, key);
      api.cleanKeyFromUrl();
      setSelectedBoardId(boardId);
    } else if (boardId) {
      // Direct board link without key (read-only)
      setSelectedBoardId(boardId);
    }
  }, []);

  // Load public boards list
  const loadBoards = useCallback(async () => {
    try {
      const { data } = await api.listBoards(showArchived);
      setBoards(data.boards || data || []);
    } catch (err) {
      console.error('Failed to load boards:', err);
    }
  }, [showArchived]);

  useEffect(() => { loadBoards(); }, [loadBoards]);

  // Load selected board detail
  useEffect(() => {
    if (!selectedBoardId) { setBoardDetail(null); setLoadError(null); return; }
    setLoadError(null);
    (async () => {
      try {
        const { data } = await api.getBoard(selectedBoardId);
        setBoardDetail(data);
      } catch (err) {
        console.error('Failed to load board:', err);
        setLoadError(err.status === 404 ? 'Board not found.' : 'Failed to load board.');
        setBoardDetail(null);
      }
    })();
  }, [selectedBoardId]);

  const canEdit = selectedBoardId ? api.hasBoardKey(selectedBoardId) : false;

  const handleBoardCreated = (newBoardId) => {
    loadBoards();
    if (newBoardId) setSelectedBoardId(newBoardId);
  };

  const handleOpenDirect = (boardId) => {
    setSelectedBoardId(boardId);
  };

  return (
    <div style={styles.app}>
      <div style={styles.header}>
        <div style={styles.logo} onClick={() => { setSelectedBoardId(null); setBoardDetail(null); }}>
          📋 Kanban
        </div>
        <div style={styles.headerRight}>
          {selectedBoardId && (
            <span style={styles.modeBadge(canEdit)}>
              {canEdit ? '✏️ Edit' : '👁️ View'}
            </span>
          )}
        </div>
      </div>

      <div style={styles.main}>
        <div style={styles.sidebar}>
          <div style={styles.sidebarHeader}>
            <span>Public Boards</span>
            <button style={styles.btnSmall} onClick={() => setShowCreateBoard(true)}>+ New</button>
          </div>
          {boards.map(b => (
            <div
              key={b.id}
              style={styles.boardItem(selectedBoardId === b.id)}
              onClick={() => setSelectedBoardId(b.id)}
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

          <div style={{ borderTop: '1px solid #334155', marginTop: 'auto' }}>
            <div style={{ padding: '8px 16px' }}>
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
            <div style={{ padding: '0 16px 4px', fontSize: '0.7rem', color: '#475569' }}>
              Open by ID:
            </div>
            <DirectBoardInput onOpen={handleOpenDirect} />
          </div>
        </div>

        {boardDetail ? (
          <BoardView board={boardDetail} canEdit={canEdit} onRefresh={() => setSelectedBoardId(s => s)} />
        ) : loadError ? (
          <div style={{ ...styles.boardContent, ...styles.empty, justifyContent: 'center', display: 'flex', alignItems: 'center' }}>
            <div>
              <p style={{ fontSize: '1.1rem', marginBottom: '8px', color: '#ef4444' }}>{loadError}</p>
              <p style={{ fontSize: '0.85rem' }}>Check the board ID and try again.</p>
            </div>
          </div>
        ) : (
          <div style={{ ...styles.boardContent, ...styles.empty, justifyContent: 'center', display: 'flex', alignItems: 'center' }}>
            <div>
              <p style={{ fontSize: '1.5rem', marginBottom: '8px' }}>📋 Kanban</p>
              <p style={{ color: '#94a3b8', marginBottom: '4px' }}>Humans Not Required</p>
              <p style={{ fontSize: '0.85rem', maxWidth: '400px', lineHeight: '1.5' }}>
                Select a public board, open one by ID, or create a new one.
                <br />
                <span style={{ color: '#64748b', fontSize: '0.8rem' }}>
                  No signup required — create a board and get a management link.
                </span>
              </p>
            </div>
          </div>
        )}
      </div>

      {showCreateBoard && (
        <CreateBoardModal
          onClose={() => setShowCreateBoard(false)}
          onCreated={handleBoardCreated}
        />
      )}
    </div>
  );
}

export default App;
