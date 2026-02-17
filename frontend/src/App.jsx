import { useState, useEffect, useCallback } from 'react';
import * as api from './api';
import styles from './styles';
import { useBreakpoint } from './hooks';
import IdentityBadge from './components/IdentityBadge';
import LiveIndicator from './components/LiveIndicator';
import { AccessIndicator } from './components/AccessIndicator';
import { CreateBoardModal } from './components/BoardModals';
import BoardView from './components/BoardView';
import WelcomePage, { DirectBoardInput } from './components/WelcomePage';

function App() {
  const { isMobile, isCompact } = useBreakpoint();
  const collapseSidebar = isCompact;
  const [myBoards, setMyBoards] = useState(() => api.getMyBoards());
  const [selectedBoardId, setSelectedBoardId] = useState(null);
  const [boardDetail, setBoardDetail] = useState(null);
  const [showCreateBoard, setShowCreateBoard] = useState(false);
  const [loadError, setLoadError] = useState(null);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [sseStatus, setSseStatus] = useState('initial');

  const refreshMyBoards = useCallback(() => setMyBoards(api.getMyBoards()), []);

  const [pendingTaskId, setPendingTaskId] = useState(null);

  useEffect(() => {
    const { boardId, key, taskId } = api.extractKeyFromUrl();
    if (boardId && key) {
      api.setBoardKey(boardId, key);
      api.cleanKeyFromUrl();
      setSelectedBoardId(boardId);
    } else if (boardId) {
      setSelectedBoardId(boardId);
    }
    if (taskId) setPendingTaskId(taskId);
  }, []);

  const loadBoardDetail = useCallback(async (boardId) => {
    const id = boardId || selectedBoardId;
    if (!id) { setBoardDetail(null); setLoadError(null); setSseStatus('initial'); return; }
    setLoadError(null);
    try {
      const { data } = await api.getBoard(id);
      setBoardDetail(data);
      api.addMyBoard(id, data.name || 'Untitled Board');
      refreshMyBoards();
    } catch (err) {
      console.error('Failed to load board:', err);
      setLoadError(err.status === 404 ? 'Board not found.' : 'Failed to load board.');
      setBoardDetail(null);
    }
  }, [selectedBoardId, refreshMyBoards]);

  useEffect(() => {
    if (!selectedBoardId) { setBoardDetail(null); setLoadError(null); return; }
    loadBoardDetail(selectedBoardId);
  }, [selectedBoardId, loadBoardDetail]);

  const [keyVersion, setKeyVersion] = useState(0);
  const canEdit = selectedBoardId ? api.hasBoardKey(selectedBoardId) : false;
  // eslint-disable-next-line no-unused-vars
  void keyVersion;

  const handleKeyUpgraded = useCallback(() => {
    setKeyVersion(v => v + 1);
    if (selectedBoardId) loadBoardDetail(selectedBoardId);
  }, [selectedBoardId, loadBoardDetail]);

  const handleBoardCreated = (newBoardId) => {
    if (newBoardId) setSelectedBoardId(newBoardId);
  };

  const handleOpenDirect = (boardId) => {
    setSelectedBoardId(boardId);
    setSidebarOpen(false);
  };

  const handleSelectBoard = (boardId) => {
    setSelectedBoardId(boardId);
    if (collapseSidebar) setSidebarOpen(false);
  };

  const handleRemoveMyBoard = (e, boardId) => {
    e.stopPropagation();
    api.removeMyBoard(boardId);
    refreshMyBoards();
    if (selectedBoardId === boardId) {
      setSelectedBoardId(null);
      setBoardDetail(null);
    }
  };

  return (
    <div style={styles.app(isMobile)}>
      <div style={styles.header(isMobile)}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flex: isCompact ? '1 1 0' : undefined }}>
          {collapseSidebar && (
            <button
              style={styles.menuBtn}
              onClick={() => setSidebarOpen(o => !o)}
              aria-label={sidebarOpen ? 'Close sidebar' : 'Open sidebar'}
            >
              <svg width="18" height="18" viewBox="0 0 18 18" fill="none" style={{ display: 'block' }}>
                <rect
                  y={sidebarOpen ? 8 : 2} width="18" height="2" rx="1" fill="currentColor"
                  style={{ transition: 'all 0.25s ease', transformOrigin: 'center',
                    transform: sidebarOpen ? 'rotate(45deg)' : 'rotate(0)' }}
                />
                <rect
                  y="8" width="18" height="2" rx="1" fill="currentColor"
                  style={{ transition: 'opacity 0.2s ease', opacity: sidebarOpen ? 0 : 1 }}
                />
                <rect
                  y={sidebarOpen ? 8 : 14} width="18" height="2" rx="1" fill="currentColor"
                  style={{ transition: 'all 0.25s ease', transformOrigin: 'center',
                    transform: sidebarOpen ? 'rotate(-45deg)' : 'rotate(0)' }}
                />
              </svg>
            </button>
          )}
          {isCompact && selectedBoardId && canEdit && (
            <IdentityBadge isMobile={isMobile} />
          )}
          {!isCompact && (
            <div style={styles.logo} onClick={() => { setSelectedBoardId(null); setBoardDetail(null); }}>
              <img src="/logo.svg" alt="" style={styles.logoImg} />
              Kanban
            </div>
          )}
        </div>
        {isCompact && (
          <div style={{ ...styles.logo, flex: '0 0 auto' }} onClick={() => { setSelectedBoardId(null); setBoardDetail(null); }}>
            <img src="/logo.svg" alt="" style={styles.logoImg} />
            Kanban
          </div>
        )}
        <div style={{ ...styles.headerRight, flex: isCompact ? '1 1 0' : undefined, justifyContent: isCompact ? 'flex-end' : undefined }}>
          {!isCompact && selectedBoardId && <LiveIndicator status={sseStatus} isMobile={false} />}
          {!isCompact && selectedBoardId && canEdit && (
            <IdentityBadge isMobile={isMobile} />
          )}
          {isCompact && selectedBoardId && <LiveIndicator status={sseStatus} isMobile={true} />}
          {selectedBoardId && (
            <AccessIndicator boardId={selectedBoardId} canEdit={canEdit} isMobile={isMobile} onKeyUpgraded={handleKeyUpgraded} />
          )}
        </div>
      </div>

      <div style={styles.main(isMobile)}>
        {collapseSidebar && sidebarOpen && (
          <div style={styles.sidebarOverlay} onClick={() => setSidebarOpen(false)} />
        )}

        <div style={styles.sidebar(collapseSidebar, sidebarOpen)}>
          <div style={styles.sidebarHeader}>
            <span>My Boards</span>
            <button style={styles.btnSmall} onClick={() => { setShowCreateBoard(true); setSidebarOpen(false); }}>+ New</button>
          </div>
          {myBoards.map(b => {
            const hasKey = api.hasBoardKey(b.id);
            return (
              <div
                key={b.id}
                style={{
                  ...styles.boardItem(selectedBoardId === b.id),
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                }}
                onClick={() => handleSelectBoard(b.id)}
              >
                <span title={hasKey ? 'Full access' : 'View only'} style={{ fontSize: '0.7rem', flexShrink: 0, opacity: 0.7 }}>
                  {hasKey ? '✏️' : '👁'}
                </span>
                <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{b.name}</span>
                <button
                  onClick={(e) => handleRemoveMyBoard(e, b.id)}
                  title="Remove from My Boards"
                  style={{
                    background: 'none',
                    border: 'none',
                    color: '#64748b',
                    cursor: 'pointer',
                    padding: '0 2px',
                    fontSize: '0.7rem',
                    flexShrink: 0,
                    lineHeight: 1,
                    opacity: 0.5,
                    transition: 'opacity 0.15s',
                  }}
                  onMouseEnter={e => e.currentTarget.style.opacity = '1'}
                  onMouseLeave={e => e.currentTarget.style.opacity = '0.5'}
                >
                  ✕
                </button>
              </div>
            );
          })}
          {myBoards.length === 0 && (
            <div style={{ ...styles.empty, padding: '20px 16px', fontSize: '0.8rem' }}>
              No boards yet. Create one or open by ID.
            </div>
          )}

          <div style={{ borderTop: '1px solid #334155', marginTop: 'auto', padding: '12px' }}>
            <DirectBoardInput onOpen={handleOpenDirect} />
            <button
              onClick={() => { setSelectedBoardId(null); setBoardDetail(null); if (collapseSidebar) setSidebarOpen(false); }}
              style={{
                background: 'transparent',
                color: '#a5b4fc',
                border: '1px solid #334155',
                borderRadius: '4px',
                padding: '5px 10px',
                fontSize: '0.75rem',
                cursor: 'pointer',
                width: '100%',
                marginTop: '8px',
                height: '32px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                gap: '6px',
                transition: 'background 0.15s, border-color 0.15s',
              }}
              onMouseEnter={e => { e.currentTarget.style.background = '#6366f122'; e.currentTarget.style.borderColor = '#6366f155'; }}
              onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.borderColor = '#334155'; }}
            >
              🌐 Browse Public Boards
            </button>
          </div>
        </div>

        {boardDetail ? (
          <BoardView board={boardDetail} canEdit={canEdit} onRefresh={() => loadBoardDetail()} onBoardRefresh={() => loadBoardDetail()} onBoardListRefresh={() => {}} isMobile={isMobile} onSseStatusChange={setSseStatus} pendingTaskId={pendingTaskId} onPendingTaskHandled={() => setPendingTaskId(null)} />
        ) : loadError ? (
          <div style={{ ...styles.boardContent(isMobile), ...styles.empty, justifyContent: 'center', display: 'flex', alignItems: 'center' }}>
            <div>
              <p style={{ fontSize: '1.1rem', marginBottom: '8px', color: '#ef4444' }}>{loadError}</p>
              <p style={{ fontSize: '0.85rem' }}>Check the board ID and try again.</p>
            </div>
          </div>
        ) : (
          <WelcomePage
            onSelectBoard={handleOpenDirect}
            onCreateBoard={() => setShowCreateBoard(true)}
            isMobile={isMobile}
          />
        )}
      </div>

      {showCreateBoard && (
        <CreateBoardModal
          onClose={() => setShowCreateBoard(false)}
          onCreated={handleBoardCreated}
          isMobile={isMobile}
        />
      )}
      <footer style={{ textAlign: 'center', padding: '8px 16px', fontSize: '0.65rem', color: '#475569', flexShrink: 0 }}>
        Made for AI, by AI.{' '}
        <a href="https://github.com/Humans-Not-Required" target="_blank" rel="noopener noreferrer" style={{ color: '#6366f1', textDecoration: 'none' }}>Humans not required</a>.
      </footer>
    </div>
  );
}

export default App;
