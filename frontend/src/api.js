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

const archiveBoard = (id) =>
  request(`/boards/${id}/archive`, { method: 'POST', boardId: id });

const unarchiveBoard = (id) =>
  request(`/boards/${id}/unarchive`, { method: 'POST', boardId: id });

// ---- Columns ----

const addColumn = (boardId, body) =>
  request(`/boards/${boardId}/columns`, { method: 'POST', body, boardId });

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

const deleteTask = (boardId, taskId) =>
  request(`/boards/${boardId}/tasks/${taskId}`, { method: 'DELETE', boardId });

const moveTask = (boardId, taskId, columnId) =>
  request(`/boards/${boardId}/tasks/${taskId}/move/${columnId}`, { method: 'POST', boardId });

const claimTask = (boardId, taskId) => {
  const name = getDisplayName();
  const agentParam = name ? `?agent=${encodeURIComponent(name)}` : '';
  return request(`/boards/${boardId}/tasks/${taskId}/claim${agentParam}`, { method: 'POST', boardId });
};

const releaseTask = (boardId, taskId) =>
  request(`/boards/${boardId}/tasks/${taskId}/release`, { method: 'POST', boardId });

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

// ---- Health ----

const health = () => request('/health');

export {
  getBoardKey, setBoardKey, removeBoardKey, hasBoardKey,
  getDisplayName, setDisplayName,
  extractKeyFromUrl, cleanKeyFromUrl,
  listBoards, getBoard, createBoard, archiveBoard, unarchiveBoard,
  addColumn,
  listTasks, getTask, createTask, updateTask, deleteTask, moveTask, claimTask, releaseTask,
  searchTasks,
  getTaskEvents, commentOnTask,
  health,
};
