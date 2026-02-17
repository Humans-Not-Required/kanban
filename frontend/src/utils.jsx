import * as api from './api';

// ---- UTC timestamp parsing ----
// API returns timestamps like "2026-02-09 16:42:27" (UTC, no timezone marker).
// Ensure they're always parsed as UTC so the browser displays in local timezone.
export function parseUTC(ts) {
  if (!ts) return new Date(NaN);
  let s = String(ts).trim();
  // Replace space separator with 'T' for ISO 8601 compliance
  if (/^\d{4}-\d{2}-\d{2} /.test(s)) s = s.replace(' ', 'T');
  // Append 'Z' if no timezone info present
  if (!s.includes('Z') && !s.includes('+') && !/T\d{2}:\d{2}(:\d{2})?[-+]/.test(s)) s += 'Z';
  return new Date(s);
}

// ---- Label normalization ----
// Lowercase, trim, collapse multiple spaces, replace spaces with dashes
export function normalizeLabel(label) {
  return label.toLowerCase().trim().replace(/\s+/g, '-').replace(/-+/g, '-').replace(/^-|-$/g, '');
}
export function normalizeLabels(labelsStr) {
  if (!labelsStr || !labelsStr.trim()) return [];
  return labelsStr.split(',').map(l => normalizeLabel(l)).filter(Boolean);
}

// ---- Priority helpers ----
export function priorityColor(p) {
  // Handle both string and integer priorities
  if (p === 'critical' || p >= 3) return '#ef4444';
  if (p === 'high' || p === 2) return '#f97316';
  if (p === 'medium' || p === 1) return '#eab308';
  if (p === 'low' || p === 0) return '#22c55e';
  return '#64748b';
}

export function priorityLabel(p) {
  if (p === 'critical' || p >= 3) return 'critical';
  if (p === 'high' || p === 2) return 'high';
  if (p === 'medium' || p === 1) return 'medium';
  if (p === 'low' || p === 0) return 'low';
  return String(p);
}

export const PRIORITY_OPTIONS = [
  { value: 3, label: 'Critical', color: '#ef4444' },
  { value: 2, label: 'High', color: '#f97316' },
  { value: 1, label: 'Medium', color: '#eab308' },
  { value: 0, label: 'Low', color: '#22c55e' },
];

// ---- Copy to clipboard helper ----
export function copyToClipboard(text) {
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

// ---- @Mention Rendering ----
/** Render text with @mentions highlighted. Returns array of React elements. */
export function renderWithMentions(text) {
  if (!text) return text;
  // Match @"Quoted Name" or @word-chars
  const mentionRegex = /@"([^"]+)"|@([\w._-]+)/g;
  const parts = [];
  let lastIndex = 0;
  let match;
  let key = 0;
  while ((match = mentionRegex.exec(text)) !== null) {
    if (match.index > lastIndex) {
      parts.push(text.slice(lastIndex, match.index));
    }
    const name = match[1] || match[2];
    const displayName = api.getDisplayName();
    const isMe = displayName && name.toLowerCase() === displayName.toLowerCase();
    // eslint-disable-next-line react/jsx-key
    parts.push(
      <span key={key++} style={{
        color: isMe ? '#fbbf24' : '#818cf8',
        fontWeight: 600,
        cursor: 'default',
      }} title={`@${name}`}>@{name}</span>
    );
    lastIndex = match.index + match[0].length;
  }
  if (lastIndex < text.length) {
    parts.push(text.slice(lastIndex));
  }
  return parts.length > 0 ? parts : text;
}

// ---- Activity helpers ----
export const LAST_VISIT_KEY = (boardId) => `kanban_last_visit_${boardId}`;

export function getLastVisit(boardId) {
  try { return localStorage.getItem(LAST_VISIT_KEY(boardId)); } catch { return null; }
}

export function setLastVisit(boardId) {
  try { localStorage.setItem(LAST_VISIT_KEY(boardId), new Date().toISOString()); } catch {}
}

export function formatEventDescription(event) {
  const { event_type, actor, data, task_title } = event;
  const who = actor || 'Someone';
  const title = task_title || '(untitled)';
  const truncTitle = title.length > 40 ? title.slice(0, 37) + '...' : title;

  switch (event_type) {
    case 'created': return `${who} created "${truncTitle}"`;
    case 'updated': return `${who} updated "${truncTitle}"`;
    case 'comment': {
      const msg = data?.message || '';
      const preview = msg.length > 60 ? msg.slice(0, 57) + '...' : msg;
      return `${who} commented on "${truncTitle}": ${preview}`;
    }
    case 'moved': {
      const to = data?.to_column || '';
      return `${who} moved "${truncTitle}"${to ? ` → ${to}` : ''}`;
    }
    case 'claimed': return `${who} claimed "${truncTitle}"`;
    case 'released': return `${who} released "${truncTitle}"`;
    case 'deleted': return `${who} deleted "${truncTitle}"`;
    case 'archived': return `${who} archived "${truncTitle}"`;
    case 'unarchived': return `${who} unarchived "${truncTitle}"`;
    default: return `${who} ${event_type} "${truncTitle}"`;
  }
}

export function formatTimeAgo(dateStr) {
  const now = new Date();
  const d = parseUTC(dateStr);
  const diff = Math.floor((now - d) / 1000);
  if (diff < 60) return 'just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  return d.toLocaleDateString();
}

export function eventIcon(type) {
  switch (type) {
    case 'created': return '✨';
    case 'updated': return '✏️';
    case 'comment': return '💬';
    case 'moved': return '➡️';
    case 'claimed': return '🙋';
    case 'released': return '🔓';
    case 'deleted': return '🗑️';
    case 'archived': return '📦';
    case 'unarchived': return '📤';
    default: return '📌';
  }
}

// ---- Webhook events ----
export const WEBHOOK_EVENTS = [
  'task.created', 'task.updated', 'task.deleted',
  'task.moved', 'task.claimed', 'task.released', 'task.comment',
];

export const TASKS_PER_PAGE = 20;
