const BASE = '/api/v1';

function getKey() {
  return localStorage.getItem('kanban_api_key') || '';
}

function setKey(key) {
  localStorage.setItem('kanban_api_key', key);
}

async function request(path, opts = {}) {
  const key = getKey();
  const headers = { ...(opts.headers || {}) };
  if (key) headers['Authorization'] = `Bearer ${key}`;
  if (opts.body && typeof opts.body === 'object') {
    headers['Content-Type'] = 'application/json';
    opts.body = JSON.stringify(opts.body);
  }
  const res = await fetch(`${BASE}${path}`, { ...opts, headers });
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

// Boards
const listBoards = (includeArchived = false) =>
  request(`/boards${includeArchived ? '?include_archived=true' : ''}`);
const getBoard = (id) => request(`/boards/${id}`);
const createBoard = (body) => request('/boards', { method: 'POST', body });
const archiveBoard = (id) => request(`/boards/${id}/archive`, { method: 'POST' });
const unarchiveBoard = (id) => request(`/boards/${id}/unarchive`, { method: 'POST' });

// Columns
const addColumn = (boardId, body) =>
  request(`/boards/${boardId}/columns`, { method: 'POST', body });

// Tasks
const listTasks = (boardId, params = '') => request(`/boards/${boardId}/tasks${params ? '?' + params : ''}`);
const getTask = (boardId, taskId) => request(`/boards/${boardId}/tasks/${taskId}`);
const createTask = (boardId, body) =>
  request(`/boards/${boardId}/tasks`, { method: 'POST', body });
const updateTask = (boardId, taskId, body) =>
  request(`/boards/${boardId}/tasks/${taskId}`, { method: 'PATCH', body });
const deleteTask = (boardId, taskId) =>
  request(`/boards/${boardId}/tasks/${taskId}`, { method: 'DELETE' });
const moveTask = (boardId, taskId, columnId) =>
  request(`/boards/${boardId}/tasks/${taskId}/move/${columnId}`, { method: 'POST' });
const claimTask = (boardId, taskId) =>
  request(`/boards/${boardId}/tasks/${taskId}/claim`, { method: 'POST' });
const releaseTask = (boardId, taskId) =>
  request(`/boards/${boardId}/tasks/${taskId}/release`, { method: 'POST' });

// Search
const searchTasks = (boardId, query, filters = '') =>
  request(`/boards/${boardId}/tasks/search?q=${encodeURIComponent(query)}${filters ? '&' + filters : ''}`);

// Health
const health = () => request('/health');

export {
  getKey, setKey,
  listBoards, getBoard, createBoard, archiveBoard, unarchiveBoard,
  addColumn,
  listTasks, getTask, createTask, updateTask, deleteTask, moveTask, claimTask, releaseTask,
  searchTasks,
  health,
};
