const BASE = '/api/v1';

// ---- Per-board key storage ----

function getBoardKey(boardId) {
  if (!boardId) return '';
  return localStorage.getItem(`kanban_key_${boardId}`) || '';
}

function setBoardKey(boardId, key) {
  if (boardId && key) {
    localStorage.setItem(`kanban_key_${boardId}`, key);
  }
}

function removeBoardKey(boardId) {
  localStorage.removeItem(`kanban_key_${boardId}`);
}

function hasBoardKey(boardId) {
  return !!getBoardKey(boardId);
}

// ---- Identity (display name) storage ----

function getDisplayName() {
  return localStorage.getItem('kanban_display_name') || '';
}

function setDisplayName(name) {
  if (name) {
    localStorage.setItem('kanban_display_name', name.trim());
  } else {
    localStorage.removeItem('kanban_display_name');
  }
}

// ---- URL param helpers ----

/** Extract ?key= from current URL and return { boardId, key } if present */
function extractKeyFromUrl() {
  const params = new URLSearchParams(window.location.search);
  const key = params.get('key');
  // Try to extract board ID from the path: /board/{id}
  const pathMatch = window.location.pathname.match(/\/board\/([a-f0-9-]+)/i);
  const boardId = pathMatch ? pathMatch[1] : null;
  return { boardId, key };
}

/** Remove ?key= from URL without reload (security: don't leave token visible) */
function cleanKeyFromUrl() {
  const url = new URL(window.location.href);
  if (url.searchParams.has('key')) {
    url.searchParams.delete('key');
    window.history.replaceState({}, '', url.toString());
  }
}

// ---- API request ----

async function request(path, opts = {}) {
  const { boardId, ...fetchOpts } = opts;
  const headers = { ...(fetchOpts.headers || {}) };

  // Use per-board key if boardId is provided
  const key = boardId ? getBoardKey(boardId) : '';
  if (key) headers['Authorization'] = `Bearer ${key}`;

  if (fetchOpts.body && typeof fetchOpts.body === 'object') {
    headers['Content-Type'] = 'application/json';
    fetchOpts.body = JSON.stringify(fetchOpts.body);
  }

  const res = await fetch(`${BASE}${path}`, { ...fetchOpts, headers });
  const rateLimit = {
    limit: res.headers.get('X-RateLimit-Limit'),
    remaining: res.headers.get('X-RateLimit-Remaining'),
    reset: res.headers.get('X-RateLimit-Reset'),
  };

  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }));
    throw { status: res.status, ...err, rateLimit };
  }

  const data = await res.json().catch(() => null);
  return { data, rateLimit };
}

// ---- Boards ----

const listBoards = (includeArchived = false) =>
  request(`/boards${includeArchived ? '?include_archived=true' : ''}`);

const getBoard = (id) => request(`/boards/${id}`);

const createBoard = async (body) => {
  // Board creation requires no auth
  const result = await request('/boards', { method: 'POST', body });
  // Store the manage key for this board
  if (result.data && result.data.board_id && result.data.manage_key) {
    setBoardKey(result.data.board_id, result.data.manage_key);
  }
  return result;
};

const updateBoard = (id, body) =>
  request(`/boards/${id}`, { method: 'PATCH', body, boardId: id });

const archiveBoard = (id) =>
  request(`/boards/${id}/archive`, { method: 'POST', boardId: id });

const unarchiveBoard = (id) =>
  request(`/boards/${id}/unarchive`, { method: 'POST', boardId: id });

// ---- Columns ----

const addColumn = (boardId, body) =>
  request(`/boards/${boardId}/columns`, { method: 'POST', body, boardId });

const updateColumn = (boardId, columnId, body) =>
  request(`/boards/${boardId}/columns/${columnId}`, { method: 'PATCH', body, boardId });

const deleteColumn = (boardId, columnId) =>
  request(`/boards/${boardId}/columns/${columnId}`, { method: 'DELETE', boardId });

const reorderColumns = (boardId, columnIds) =>
  request(`/boards/${boardId}/columns/reorder`, { method: 'POST', body: { column_ids: columnIds }, boardId });

// ---- Tasks ----

const listTasks = (boardId, params = '') =>
  request(`/boards/${boardId}/tasks${params ? '?' + params : ''}`, { boardId });

const getTask = (boardId, taskId) =>
  request(`/boards/${boardId}/tasks/${taskId}`, { boardId });

const createTask = (boardId, body) => {
  const name = getDisplayName();
  if (name && !body.actor_name) body.actor_name = name;
  return request(`/boards/${boardId}/tasks`, { method: 'POST', body, boardId });
};

const updateTask = (boardId, taskId, body) => {
  const name = getDisplayName();
  if (name && !body.actor_name) body.actor_name = name;
  return request(`/boards/${boardId}/tasks/${taskId}`, { method: 'PATCH', body, boardId });
};

const deleteTask = (boardId, taskId) => {
  const name = getDisplayName();
  const actorParam = name ? `?actor=${encodeURIComponent(name)}` : '';
  return request(`/boards/${boardId}/tasks/${taskId}${actorParam}`, { method: 'DELETE', boardId });
};

const archiveTask = (boardId, taskId) => {
  const name = getDisplayName();
  const actorParam = name ? `?actor=${encodeURIComponent(name)}` : '';
  return request(`/boards/${boardId}/tasks/${taskId}/archive${actorParam}`, { method: 'POST', boardId });
};

const unarchiveTask = (boardId, taskId) => {
  const name = getDisplayName();
  const actorParam = name ? `?actor=${encodeURIComponent(name)}` : '';
  return request(`/boards/${boardId}/tasks/${taskId}/unarchive${actorParam}`, { method: 'POST', boardId });
};

const moveTask = (boardId, taskId, columnId) => {
  const name = getDisplayName();
  const actorParam = name ? `?actor=${encodeURIComponent(name)}` : '';
  return request(`/boards/${boardId}/tasks/${taskId}/move/${columnId}${actorParam}`, { method: 'POST', boardId });
};

const claimTask = (boardId, taskId) => {
  const name = getDisplayName();
  const agentParam = name ? `?agent=${encodeURIComponent(name)}` : '';
  return request(`/boards/${boardId}/tasks/${taskId}/claim${agentParam}`, { method: 'POST', boardId });
};

const releaseTask = (boardId, taskId) => {
  const name = getDisplayName();
  const actorParam = name ? `?actor=${encodeURIComponent(name)}` : '';
  return request(`/boards/${boardId}/tasks/${taskId}/release${actorParam}`, { method: 'POST', boardId });
};

// ---- Search ----

const searchTasks = (boardId, query, filters = '') =>
  request(`/boards/${boardId}/tasks/search?q=${encodeURIComponent(query)}${filters ? '&' + filters : ''}`, { boardId });

// ---- Task Events & Comments ----

const getTaskEvents = (boardId, taskId) =>
  request(`/boards/${boardId}/tasks/${taskId}/events`, { boardId });

const commentOnTask = (boardId, taskId, message, actorName) => {
  const name = actorName || getDisplayName() || undefined;
  return request(`/boards/${boardId}/tasks/${taskId}/comment`, {
    method: 'POST',
    body: { message, actor_name: name },
    boardId,
  });
};

// ---- SSE (Server-Sent Events) ----

/**
 * Subscribe to real-time board events via SSE.
 * Returns an object with { close() } to unsubscribe.
 * 
 * @param {string} boardId - Board UUID
 * @param {function} onEvent - Callback: ({ event, data }) => void
 * @param {function} [onStatus] - Optional status callback: ('connected' | 'disconnected' | 'error') => void
 * @returns {{ close: () => void }}
 */
function subscribeToBoardEvents(boardId, onEvent, onStatus) {
  const url = `${BASE}/boards/${boardId}/events/stream`;
  let es = null;
  let closed = false;
  let reconnectTimer = null;
  let reconnectDelay = 1000;

  function connect() {
    if (closed) return;
    es = new EventSource(url);

    es.onopen = () => {
      reconnectDelay = 1000; // reset backoff on successful connection
      if (onStatus) onStatus('connected');
    };

    es.onerror = () => {
      if (closed) return;
      if (onStatus) onStatus('disconnected');
      es.close();
      // Reconnect with exponential backoff (max 30s)
      reconnectTimer = setTimeout(() => {
        reconnectDelay = Math.min(reconnectDelay * 2, 30000);
        connect();
      }, reconnectDelay);
    };

    // Listen for all known event types
    const eventTypes = [
      'task.created', 'task.updated', 'task.deleted',
      'task.moved', 'task.claimed', 'task.released',
      'task.reordered', 'task.comment', 'warning',
    ];

    eventTypes.forEach(type => {
      es.addEventListener(type, (e) => {
        try {
          const data = JSON.parse(e.data);
          onEvent({ event: type, data });
        } catch {
          onEvent({ event: type, data: e.data });
        }
      });
    });
  }

  connect();

  return {
    close() {
      closed = true;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      if (es) es.close();
      if (onStatus) onStatus('disconnected');
    },
  };
}

// ---- Webhooks ----

const listWebhooks = (boardId) =>
  request(`/boards/${boardId}/webhooks`, { boardId });

const createWebhook = (boardId, body) =>
  request(`/boards/${boardId}/webhooks`, { method: 'POST', body, boardId });

const updateWebhook = (boardId, webhookId, body) =>
  request(`/boards/${boardId}/webhooks/${webhookId}`, { method: 'PATCH', body, boardId });

const deleteWebhook = (boardId, webhookId) =>
  request(`/boards/${boardId}/webhooks/${webhookId}`, { method: 'DELETE', boardId });

// ---- Activity ----

const getBoardActivity = (boardId, { since, limit } = {}) => {
  const params = new URLSearchParams();
  if (since) params.set('since', since);
  if (limit) params.set('limit', String(limit));
  const qs = params.toString();
  return request(`/boards/${boardId}/activity${qs ? '?' + qs : ''}`);
};

// ---- My Boards (localStorage) ----

const MY_BOARDS_KEY = 'kanban_my_boards';

/** Get the user's board list from localStorage: [{id, name, lastAccessed}] */
function getMyBoards() {
  try {
    return JSON.parse(localStorage.getItem(MY_BOARDS_KEY) || '[]');
  } catch {
    return [];
  }
}

/** Add or update a board in the user's list */
function addMyBoard(id, name) {
  const boards = getMyBoards();
  const idx = boards.findIndex(b => b.id === id);
  const entry = { id, name: name || 'Untitled Board', lastAccessed: new Date().toISOString() };
  if (idx >= 0) {
    boards[idx] = { ...boards[idx], ...entry };
  } else {
    boards.unshift(entry);
  }
  localStorage.setItem(MY_BOARDS_KEY, JSON.stringify(boards));
}

/** Remove a board from the user's list */
function removeMyBoard(id) {
  const boards = getMyBoards().filter(b => b.id !== id);
  localStorage.setItem(MY_BOARDS_KEY, JSON.stringify(boards));
}

// ---- Key validation ----

// Try a no-op PATCH to verify a manage key is valid for a board
const validateKey = async (boardId, key) => {
  try {
    const res = await fetch(`${BASE}/boards/${boardId}`, {
      method: 'PATCH',
      headers: {
        'Authorization': `Bearer ${key}`,
        'Content-Type': 'application/json',
      },
      body: '{}',
    });
    return res.ok;
  } catch {
    return false;
  }
};

// ---- Health ----

const health = () => request('/health');

export {
  getBoardKey, setBoardKey, removeBoardKey, hasBoardKey,
  getDisplayName, setDisplayName,
  extractKeyFromUrl, cleanKeyFromUrl,
  getMyBoards, addMyBoard, removeMyBoard,
  listBoards, getBoard, createBoard, updateBoard, archiveBoard, unarchiveBoard,
  addColumn, updateColumn, deleteColumn, reorderColumns,
  listTasks, getTask, createTask, updateTask, deleteTask, archiveTask, unarchiveTask, moveTask, claimTask, releaseTask,
  searchTasks,
  getTaskEvents, commentOnTask,
  getBoardActivity,
  subscribeToBoardEvents,
  listWebhooks, createWebhook, updateWebhook, deleteWebhook,
  validateKey,
  health,
};
