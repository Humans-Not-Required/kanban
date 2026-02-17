import { useState, useEffect, useCallback, useRef } from "react";
import * as api from "../api";
import styles from "../styles";
import { getLastVisit } from "../utils";
import Column, { FullScreenColumnView } from "./Column";
import StyledSelect from "./StyledSelect";
import { CreateTaskModal, TaskDetailModal } from "./TaskModals";
import { BoardSettingsModal } from "./BoardModals";
import ActivityPanel from "./ActivityPanel";

function BoardView({ board, canEdit, onRefresh, onBoardRefresh, onBoardListRefresh, isMobile, onSseStatusChange, pendingTaskId, onPendingTaskHandled }) {
  const [tasks, setTasks] = useState([]);
  const [showCreate, setShowCreate] = useState(false);
  const [search, setSearch] = useState('');
  const [searchResults, setSearchResults] = useState(null);
  const searchRef = useRef({ search: '', hasResults: false });
  const [selectedTask, setSelectedTask] = useState(null);
  const [sseStatus, setSseStatus] = useState('initial');
  const [addingColumn, setAddingColumn] = useState(false);
  const [newColumnName, setNewColumnName] = useState('');
  // showWebhooks state removed — webhook button removed from UI per Jordan's request
  const [showSettings, setShowSettings] = useState(false);
  const [showActivity, setShowActivity] = useState(false);
  const [filterPriority, setFilterPriority] = useState('');
  const [filterLabel, setFilterLabel] = useState('');
  const [filterAssignee, setFilterAssignee] = useState('');
  const [filterCreatedBy, setFilterCreatedBy] = useState('');
  const [showFilters, setShowFilters] = useState(false);
  const [showArchivedTasks, setShowArchivedTasks] = useState(false);
  const [showSearchBar, setShowSearchBar] = useState(!isMobile);
  const [collapsedColumns, setCollapsedColumns] = useState({});
  const [tasksLoaded, setTasksLoaded] = useState(false);
  const [newActivityCount, setNewActivityCount] = useState(0);
  const [fullScreenColumnId, setFullScreenColumnId] = useState(null);
  const toggleColumnCollapse = useCallback((colId) => {
    setCollapsedColumns(prev => ({ ...prev, [colId]: !prev[colId] }));
  }, []);

  const loadTasks = useCallback(async () => {
    try {
      const params = showArchivedTasks ? 'archived=true' : '';
      const { data } = await api.listTasks(board.id, params);
      setTasks(data.tasks || data || []);
      setTasksLoaded(true);
    } catch (err) {
      console.error('Failed to load tasks:', err);
    }
  }, [board.id, showArchivedTasks]);

  useEffect(() => { loadTasks(); }, [loadTasks]);

  // Auto-open task from URL ?task= param
  useEffect(() => {
    if (pendingTaskId && tasksLoaded && tasks.length > 0) {
      const task = tasks.find(t => t.id === pendingTaskId);
      if (task) {
        setSelectedTask(task);
      }
      if (onPendingTaskHandled) onPendingTaskHandled();
    }
  }, [pendingTaskId, tasksLoaded, tasks]);

  // Update URL when task is selected/deselected
  useEffect(() => {
    api.setTaskInUrl(selectedTask ? selectedTask.id : null);
  }, [selectedTask]);

  // Sync selectedTask with refreshed tasks data (fixes stale view after edit/save)
  useEffect(() => {
    if (selectedTask) {
      const updated = tasks.find(t => t.id === selectedTask.id);
      if (updated && JSON.stringify(updated) !== JSON.stringify(selectedTask)) {
        setSelectedTask(updated);
      } else if (!updated && !showArchivedTasks) {
        // Task may have been deleted or archived
        setSelectedTask(null);
      }
    }
  }, [tasks]);

  // Load new activity count for the badge
  useEffect(() => {
    const lv = getLastVisit(board.id);
    if (!lv) { setNewActivityCount(0); return; }
    (async () => {
      try {
        const { data } = await api.getBoardActivity(board.id, { since: lv, limit: 100 });
        setNewActivityCount((data || []).length);
      } catch { setNewActivityCount(0); }
    })();
  }, [board.id, showActivity]);

  // SSE: subscribe to real-time board events (debounced refresh)
  useEffect(() => {
    let debounceTimer = null;
    const debouncedRefresh = () => {
      if (debounceTimer) clearTimeout(debounceTimer);
      debounceTimer = setTimeout(async () => {
        await loadTasks();
        // Also refresh search results if a search is active
        const { search: q, hasResults } = searchRef.current;
        if (hasResults && q.trim()) {
          try {
            const { data } = await api.searchTasks(board.id, q.trim());
            setSearchResults(data.tasks || []);
          } catch (err) { /* ignore */ }
        }
      }, 300);
    };
    const sub = api.subscribeToBoardEvents(
      board.id,
      (evt) => {
        // On any task event, debounce-refresh the task list
        if (evt.event !== 'warning') {
          debouncedRefresh();
        }
      },
      (status) => { setSseStatus(status); onSseStatusChange?.(status); }, // Feed status to header indicator
    );
    return () => {
      if (debounceTimer) clearTimeout(debounceTimer);
      sub.close();
    };
  }, [board.id, loadTasks]);

  // Keep ref in sync so SSE handler can check without being a dependency
  useEffect(() => {
    searchRef.current = { search, hasResults: searchResults !== null };
  }, [search, searchResults]);

  const doSearch = async () => {
    if (!search.trim()) { setSearchResults(null); return; }
    try {
      const { data } = await api.searchTasks(board.id, search.trim());
      setSearchResults(data.tasks || []);
    } catch (err) {
      console.error('Search failed:', err);
    }
  };

  // Refresh both tasks and search results (if active) so the board stays in sync
  const refreshAll = useCallback(async () => {
    await loadTasks();
    if (searchResults !== null && search.trim()) {
      try {
        const { data } = await api.searchTasks(board.id, search.trim());
        setSearchResults(data.tasks || []);
      } catch (err) {
        console.error('Search refresh failed:', err);
      }
    }
  }, [loadTasks, searchResults, search, board.id]);

  const columns = board.columns || [];
  const baseTasks = searchResults !== null ? searchResults : tasks;

  // Collect unique labels and assignees for filter dropdowns
  const allLabels = (() => {
    const counts = {};
    baseTasks.forEach(t => {
      (Array.isArray(t.labels) ? t.labels : (t.labels || '').split(',').map(l => l.trim())).filter(Boolean).forEach(l => {
        counts[l] = (counts[l] || 0) + 1;
      });
    });
    return Object.keys(counts).sort((a, b) => counts[b] - counts[a]);
  })();
  const allLabelsSorted = [...allLabels].sort((a, b) => a.localeCompare(b));
  const allAssignees = [...new Set(baseTasks.map(t => t.assigned_to || t.claimed_by).filter(Boolean))].sort();
  const allCreators = [...new Set(baseTasks.map(t => t.created_by).filter(Boolean))].sort();

  // Apply filters
  const displayTasks = baseTasks.filter(t => {
    if (filterPriority) {
      if (filterPriority === '3') { if ((t.priority || 0) < 3) return false; }
      else if (String(t.priority) !== filterPriority) return false;
    }
    if (filterLabel && !(Array.isArray(t.labels) ? t.labels : (t.labels || '').split(',').map(l => l.trim())).some(l => l.toLowerCase() === filterLabel.toLowerCase())) return false;
    if (filterAssignee && t.assigned_to !== filterAssignee && t.claimed_by !== filterAssignee) return false;
    if (filterCreatedBy && t.created_by !== filterCreatedBy) return false;
    return true;
  });
  const hasActiveFilters = filterPriority || filterLabel || filterAssignee || filterCreatedBy || showArchivedTasks;
  const searchActive = searchResults !== null || hasActiveFilters;
  const archived = !!board.archived_at;

  return (
    <div style={styles.boardContent(isMobile)}>
      <div style={styles.boardHeader(isMobile)}>
        <div style={{ minWidth: 0 }}>
          <span style={styles.boardTitle(isMobile)}>{board.name}</span>
          {archived && <span style={{ ...styles.archivedBadge, marginLeft: '10px' }}>ARCHIVED</span>}
          {board.description && (
            <p style={{ fontSize: '0.8rem', color: '#64748b', marginTop: '4px' }}>{board.description}</p>
          )}
        </div>
        {isMobile ? (
          /* Mobile: connected segmented button bar, 100% width */
          <div style={{ display: 'flex', width: '100%', borderRadius: '6px', overflow: 'hidden', border: '1px solid #475569' }}>
            <button style={{ flex: '1 1 0', background: '#334155', color: '#cbd5e1', border: 'none', borderRight: '1px solid #475569', padding: '10px 0', cursor: 'pointer', fontSize: '0.85rem', display: 'inline-flex', alignItems: 'center', justifyContent: 'center', position: 'relative' }} onClick={() => setShowActivity(true)} title="Activity Feed">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
              {newActivityCount > 0 && (
                <span style={{
                  position: 'absolute', top: '4px', right: '4px',
                  background: '#6366f1',
                  width: '8px', height: '8px',
                  borderRadius: '50%',
                }} />
              )}
            </button>
            <button style={{ flex: '1 1 0', background: '#334155', color: '#cbd5e1', border: 'none', borderRight: '1px solid #475569', padding: '10px 0', cursor: 'pointer', fontSize: '0.85rem', display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }} onClick={() => setShowSettings(true)} title="Board Settings">⚙️</button>
            <button style={{ flex: '1 1 0', background: searchActive ? '#312e81' : showSearchBar ? '#475569' : '#334155', color: searchActive ? '#a5b4fc' : '#cbd5e1', border: 'none', borderRight: '1px solid #475569', padding: '10px 0', cursor: 'pointer', fontSize: '0.85rem', display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }} onClick={() => setShowSearchBar(v => !v)} title="Search & Filter">🔍</button>
            {canEdit && !archived && (
              <button style={{ flex: '0 0 33.33%', background: '#6366f1', color: '#fff', border: 'none', padding: '10px 14px', cursor: 'pointer', fontSize: '0.9rem', fontWeight: 600, display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: '6px' }} onClick={() => setShowCreate(true)}>+ Task</button>
            )}
          </div>
        ) : (
          /* Desktop: original button layout */
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexShrink: 0, flexWrap: 'wrap' }}>
            <button style={{ ...styles.btn('secondary', false), position: 'relative' }} onClick={() => setShowActivity(true)} title="Activity Feed">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
              {newActivityCount > 0 && (
                <span style={{
                  position: 'absolute', top: '-2px', right: '-2px',
                  background: '#6366f1',
                  width: '8px', height: '8px',
                  borderRadius: '50%',
                }} />
              )}
            </button>
            <button style={styles.btn('secondary', false)} onClick={() => setShowSettings(true)} title="Board Settings">⚙️</button>
            {canEdit && !archived && (
              <button style={styles.btn('primary', false)} onClick={() => setShowCreate(true)}>+ Task</button>
            )}
          </div>
        )}
      </div>

      {showSearchBar && (
        <div style={styles.searchBar(isMobile)}>
          <div style={{ position: 'relative', flex: 1, display: 'flex', alignItems: 'center' }}>
            <input
              style={{
                ...styles.input,
                marginBottom: 0, width: '100%', paddingRight: search ? '28px' : undefined, height: '32px', padding: '4px 10px', fontSize: '16px',
                ...(searchResults !== null ? { border: '1px solid #6366f1', background: '#1e1b4b', boxShadow: '0 0 0 2px rgba(99,102,241,0.15)' } : {}),
              }}
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
          <button style={{ ...styles.btn('secondary', false), ...(searchResults !== null ? { border: '1px solid #6366f1', color: '#a5b4fc', background: '#312e81' } : {}), ...(isMobile ? { padding: '3px 8px', minWidth: '32px' } : {}) }} onClick={doSearch} title="Search">
            {isMobile ? (
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><line x1="5" y1="12" x2="19" y2="12"/><polyline points="12 5 19 12 12 19"/></svg>
            ) : 'Search'}
          </button>
          {!isMobile && (
            <button style={{ ...styles.btn('secondary', false), ...(hasActiveFilters ? { border: '1px solid #6366f1', color: '#a5b4fc', background: '#312e81' } : {}), display: 'flex', alignItems: 'center', gap: isMobile ? '0' : '5px', ...(isMobile ? { padding: '3px 8px', minWidth: '32px' } : {}) }} onClick={() => setShowFilters(f => !f)} title="Filter">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"/></svg>
              {!isMobile && 'Filter'}
            </button>
          )}
        </div>
      )}
      {showSearchBar && (isMobile || showFilters) && (
        <div style={isMobile ? { display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '8px', padding: '8px 12px', alignItems: 'center' } : { display: 'flex', gap: '8px', padding: '8px 20px', flexWrap: 'wrap', alignItems: 'center' }}>
          <StyledSelect style={{ ...styles.select, marginBottom: 0, flex: isMobile ? '1 1 auto' : 'none', minWidth: isMobile ? 0 : '120px', width: isMobile ? '100%' : undefined, ...(isMobile ? { gridColumn: 'span 2' } : {}), padding: '6px 12px', fontSize: '16px', borderRadius: '4px', background: filterPriority ? '#3b82f611' : '#0f172a', border: `1px solid ${filterPriority ? '#3b82f644' : '#334155'}`, color: filterPriority ? '#93c5fd' : '#94a3b8', cursor: 'pointer', height: '32px', lineHeight: '1' }} value={filterPriority} onChange={e => setFilterPriority(e.target.value)}>
            <option value="">Any Priority</option>
            <option value="3">🔴 Critical</option>
            <option value="2">🟠 High</option>
            <option value="1">🟡 Medium</option>
            <option value="0">🟢 Low</option>
          </StyledSelect>
          <StyledSelect style={{ ...styles.select, marginBottom: 0, flex: isMobile ? '1 1 auto' : 'none', minWidth: isMobile ? 0 : '120px', width: isMobile ? '100%' : undefined, ...(isMobile ? { gridColumn: 'span 2' } : {}), padding: '6px 12px', fontSize: '16px', borderRadius: '4px', background: filterLabel ? '#3b82f611' : '#0f172a', border: `1px solid ${filterLabel ? '#3b82f644' : '#334155'}`, color: filterLabel ? '#93c5fd' : '#94a3b8', cursor: 'pointer', height: '32px', lineHeight: '1' }} value={filterLabel} onChange={e => setFilterLabel(e.target.value)}>
            <option value="">Any Label</option>
            {allLabelsSorted.map(l => (
              <option key={l} value={l}>{l}</option>
            ))}
          </StyledSelect>
          <StyledSelect style={{ ...styles.select, marginBottom: 0, flex: isMobile ? '1 1 auto' : 'none', minWidth: isMobile ? 0 : '120px', width: isMobile ? '100%' : undefined, ...(isMobile ? { gridColumn: 'span 2' } : {}), padding: '6px 12px', fontSize: '16px', borderRadius: '4px', background: filterAssignee ? '#3b82f611' : '#0f172a', border: `1px solid ${filterAssignee ? '#3b82f644' : '#334155'}`, color: filterAssignee ? '#93c5fd' : '#94a3b8', cursor: 'pointer', height: '32px', lineHeight: '1' }} value={filterAssignee} onChange={e => setFilterAssignee(e.target.value)}>
            <option value="">Any Assignee</option>
            {allAssignees.map(a => (
              <option key={a} value={a}>{a}</option>
            ))}
          </StyledSelect>
          <StyledSelect style={{ ...styles.select, marginBottom: 0, flex: isMobile ? '1 1 auto' : 'none', minWidth: isMobile ? 0 : '120px', width: isMobile ? '100%' : undefined, ...(isMobile ? { gridColumn: 'span 2' } : {}), padding: '6px 12px', fontSize: '16px', borderRadius: '4px', background: filterCreatedBy ? '#3b82f611' : '#0f172a', border: `1px solid ${filterCreatedBy ? '#3b82f644' : '#334155'}`, color: filterCreatedBy ? '#93c5fd' : '#94a3b8', cursor: 'pointer', height: '32px', lineHeight: '1' }} value={filterCreatedBy} onChange={e => setFilterCreatedBy(e.target.value)}>
            <option value="">Created By</option>
            {allCreators.map(c => (
              <option key={c} value={c}>{c}</option>
            ))}
          </StyledSelect>
          <button
            onClick={() => setShowArchivedTasks(v => !v)}
            style={{
              ...styles.select, marginBottom: 0, flex: isMobile ? '1 1 auto' : 'none',
              ...(isMobile ? { gridColumn: 'span 1', width: '100%' } : {}),
              padding: '6px 12px', cursor: 'pointer', whiteSpace: 'nowrap',
              background: showArchivedTasks ? '#6366f133' : '#0f172a',
              color: showArchivedTasks ? '#a5b4fc' : '#94a3b8',
              border: `1px solid ${showArchivedTasks ? '#6366f155' : '#334155'}`,
              borderRadius: '4px', fontSize: isMobile ? '16px' : '0.78rem',
              height: '32px', lineHeight: '1',
            }}
          >
            📦 {isMobile ? 'Archive' : 'Archived'} {showArchivedTasks ? '✓' : ''}
          </button>
          <button
            disabled={!hasActiveFilters && !showArchivedTasks}
            style={{
              ...styles.btnSmall,
              ...(isMobile ? { gridColumn: 'span 1', width: '100%' } : {}),
              ...(!hasActiveFilters && !showArchivedTasks ? { opacity: 0.4, cursor: 'not-allowed' } : {}),
            }}
            onClick={() => { setFilterPriority(''); setFilterLabel(''); setFilterAssignee(''); setFilterCreatedBy(''); setShowArchivedTasks(false); }}
          >
            {isMobile ? 'Clear' : 'Clear Filters'}
          </button>
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
            onRefresh={refreshAll}
            onBoardRefresh={onBoardRefresh}
            archived={archived}
            onClickTask={setSelectedTask}
            isMobile={isMobile}
            allColumns={columns}
            collapsed={collapsedColumns[col.id]}
            onToggleCollapse={() => toggleColumnCollapse(col.id)}
            tasksLoaded={tasksLoaded}
            onFullScreen={() => setFullScreenColumnId(col.id)}
          />
        ))}
        {canEdit && !archived && (
          addingColumn ? (
            <div style={{ ...styles.column(false, isMobile), minWidth: isMobile ? undefined : '200px', maxWidth: isMobile ? undefined : '200px', justifyContent: 'flex-start' }}>
              <input
                autoFocus
                style={{ background: '#1e293b', color: '#e2e8f0', border: '1px solid #3b82f6', borderRadius: '4px', padding: '6px 8px', fontSize: '16px', width: '100%' }}
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

      {fullScreenColumnId && (() => {
        const fsCol = columns.find(c => c.id === fullScreenColumnId);
        return fsCol ? (
          <FullScreenColumnView
            column={fsCol}
            tasks={displayTasks}
            boardId={board.id}
            canEdit={canEdit}
            onRefresh={refreshAll}
            onClose={() => setFullScreenColumnId(null)}
            onClickTask={setSelectedTask}
            archived={archived}
          />
        ) : null;
      })()}

      {showCreate && (
        <CreateTaskModal
          boardId={board.id}
          columns={columns}
          onClose={() => setShowCreate(false)}
          onCreated={refreshAll}
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
          onRefresh={refreshAll}
          isMobile={isMobile}
          allColumns={columns}
          allLabels={allLabels}
          allAssignees={allAssignees}
          quickDoneColumnId={board.quick_done_column_id}
          quickDoneAutoArchive={board.quick_done_auto_archive}
          quickReassignColumnId={board.quick_reassign_column_id}
          quickReassignTo={board.quick_reassign_to}
        />
      )}

      {showSettings && (
        <BoardSettingsModal
          board={board}
          canEdit={canEdit}
          onClose={() => setShowSettings(false)}
          onRefresh={onBoardRefresh}
          onBoardListRefresh={onBoardListRefresh}
          isMobile={isMobile}
        />
      )}

      {showActivity && (
        <ActivityPanel
          boardId={board.id}
          onClose={() => setShowActivity(false)}
          isMobile={isMobile}
          onOpenTask={(task) => { setSelectedTask(task); setShowActivity(false); }}
        />
      )}

      {/* Webhook management removed from UI (Jordan request). API still available. */}
    </div>
  );
}


export default BoardView;
