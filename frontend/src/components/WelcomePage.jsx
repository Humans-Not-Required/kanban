import { useState, useEffect } from "react";
import * as api from "../api";
import styles from "../styles";

function DirectBoardInput({ onOpen }) {
  const [boardId, setBoardId] = useState('');

  const submit = (e) => {
    e.preventDefault();
    const id = boardId.trim();
    if (!id) return;
    const match = id.match(/\/board\/([a-f0-9-]+)/i);
    onOpen(match ? match[1] : id);
    setBoardId('');
  };

  return (
    <form onSubmit={submit} style={{ display: 'flex', gap: '6px' }}>
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

// ---- Welcome / Public Boards Discovery ----

function WelcomePage({ onSelectBoard, onCreateBoard, isMobile }) {
  const [publicBoards, setPublicBoards] = useState([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');

  useEffect(() => {
    (async () => {
      try {
        const { data } = await api.listBoards(false);
        setPublicBoards(data.boards || data || []);
      } catch (err) {
        console.error('Failed to load public boards:', err);
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const filtered = searchQuery.trim()
    ? publicBoards.filter(b =>
        b.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        (b.description || '').toLowerCase().includes(searchQuery.toLowerCase())
      )
    : publicBoards;

  const totalTasks = publicBoards.reduce((sum, b) => sum + (b.task_count || 0), 0);

  return (
    <div style={{
      flex: 1, overflowY: 'auto', display: 'flex', flexDirection: 'column',
      alignItems: 'center', padding: isMobile ? '24px 16px' : '40px 24px',
    }}>
      {/* Hero */}
      <div style={{ textAlign: 'center', marginBottom: '32px', maxWidth: '520px' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '10px', marginBottom: '8px' }}>
          <img src="/logo.svg" alt="" style={{ width: '36px', height: '36px' }} />
          <h1 style={{ color: '#f1f5f9', fontSize: isMobile ? '1.6rem' : '2rem', fontWeight: 700, margin: 0 }}>Kanban</h1>
        </div>
        <p style={{ color: '#94a3b8', fontSize: '0.95rem', marginBottom: '6px' }}>
          Humans Not Required
        </p>
        <p style={{ color: '#64748b', fontSize: '0.83rem', lineHeight: '1.5', maxWidth: '400px', margin: '0 auto' }}>
          Agent-first project boards. No signup, no accounts — just create a board and share the link.
        </p>
        <button
          style={{
            ...styles.btn('primary', isMobile),
            marginTop: '16px',
            padding: '10px 24px',
            fontSize: '0.9rem',
          }}
          onClick={onCreateBoard}
        >
          + Create a Board
        </button>
      </div>

      {/* Stats bar */}
      {!loading && publicBoards.length > 0 && (
        <div style={{
          display: 'flex', gap: '24px', justifyContent: 'center', marginBottom: '24px',
          padding: '12px 20px', background: '#1e293b', borderRadius: '8px',
          border: '1px solid #334155',
        }}>
          <div style={{ textAlign: 'center' }}>
            <div style={{ color: '#a5b4fc', fontSize: '1.2rem', fontWeight: 700 }}>{publicBoards.length}</div>
            <div style={{ color: '#64748b', fontSize: '0.7rem', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
              Public Board{publicBoards.length !== 1 ? 's' : ''}
            </div>
          </div>
          <div style={{ width: '1px', background: '#334155' }} />
          <div style={{ textAlign: 'center' }}>
            <div style={{ color: '#22c55e', fontSize: '1.2rem', fontWeight: 700 }}>{totalTasks}</div>
            <div style={{ color: '#64748b', fontSize: '0.7rem', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
              Total Tasks
            </div>
          </div>
        </div>
      )}

      {/* Public Boards Section */}
      <div style={{ width: '100%', maxWidth: '800px' }}>
        <div style={{
          display: 'flex', justifyContent: 'space-between', alignItems: 'center',
          marginBottom: '12px', gap: '12px', flexWrap: 'wrap',
        }}>
          <h2 style={{ color: '#e2e8f0', fontSize: '1rem', fontWeight: 600, margin: 0 }}>
            🌐 Public Boards
          </h2>
          {publicBoards.length > 3 && (
            <div style={{ position: 'relative', flex: isMobile ? '1 1 100%' : '0 1 240px' }}>
              <input
                style={{
                  ...styles.input, marginBottom: 0, width: '100%',
                  height: '32px', padding: '4px 10px', fontSize: '16px',
                  paddingRight: searchQuery ? '28px' : undefined,
                }}
                placeholder="Search boards..."
                value={searchQuery}
                onChange={e => setSearchQuery(e.target.value)}
              />
              {searchQuery && (
                <button
                  onClick={() => setSearchQuery('')}
                  style={{
                    position: 'absolute', right: '6px', top: '50%', transform: 'translateY(-50%)',
                    width: '20px', height: '20px', display: 'flex', alignItems: 'center', justifyContent: 'center',
                    borderRadius: '999px', background: '#0b1220', border: '1px solid #334155',
                    color: '#94a3b8', cursor: 'pointer', fontSize: '13px', padding: 0, lineHeight: 1,
                  }}
                >×</button>
              )}
            </div>
          )}
        </div>

        {loading ? (
          <div style={{ color: '#64748b', textAlign: 'center', padding: '40px 0', fontSize: '0.85rem' }}>
            Loading boards...
          </div>
        ) : filtered.length === 0 ? (
          <div style={{
            textAlign: 'center', padding: '32px 16px', color: '#475569',
            background: '#1a2332', borderRadius: '8px', border: '1px solid #334155',
          }}>
            {searchQuery
              ? <p style={{ fontSize: '0.85rem' }}>No boards matching "{searchQuery}"</p>
              : (
                <>
                  <p style={{ fontSize: '0.9rem', marginBottom: '4px' }}>No public boards yet.</p>
                  <p style={{ fontSize: '0.8rem' }}>Create the first one!</p>
                </>
              )
            }
          </div>
        ) : (
          <div style={{
            display: 'grid',
            gridTemplateColumns: isMobile ? '1fr' : 'repeat(auto-fill, minmax(240px, 1fr))',
            gap: '12px',
          }}>
            {filtered.map(board => (
              <div
                key={board.id}
                onClick={() => onSelectBoard(board.id)}
                style={{
                  background: '#1a2332', border: '1px solid #334155', borderRadius: '8px',
                  padding: '16px', cursor: 'pointer',
                  transition: 'border-color 0.15s, background 0.15s',
                }}
                onMouseEnter={e => { e.currentTarget.style.borderColor = '#6366f1'; e.currentTarget.style.background = '#1e293b'; }}
                onMouseLeave={e => { e.currentTarget.style.borderColor = '#334155'; e.currentTarget.style.background = '#1a2332'; }}
              >
                <h3 style={{
                  color: '#e2e8f0', fontSize: '0.95rem', fontWeight: 600,
                  margin: '0 0 6px 0',
                  overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                }}>
                  {board.name}
                </h3>
                {board.description && (
                  <p style={{
                    color: '#94a3b8', fontSize: '0.78rem', margin: '0 0 10px 0',
                    lineHeight: '1.4',
                    display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical',
                    overflow: 'hidden',
                  }}>
                    {board.description}
                  </p>
                )}
                <div style={{ display: 'flex', gap: '12px', alignItems: 'center', fontSize: '0.72rem', color: '#64748b' }}>
                  <span title="Tasks">📋 {board.task_count} task{board.task_count !== 1 ? 's' : ''}</span>
                  <span title="Created">{formatTimeAgo(board.created_at)}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Open by ID section */}
      <div style={{
        width: '100%', maxWidth: '800px', marginTop: '32px',
        padding: '16px 20px', background: '#1e293b', borderRadius: '8px',
        border: '1px solid #334155',
      }}>
        <div style={{
          fontSize: '0.8rem', fontWeight: 600, color: '#94a3b8',
          marginBottom: '8px',
        }}>
          Open a board by ID or URL
        </div>
        <DirectBoardInput onOpen={onSelectBoard} />
      </div>
    </div>
  );
}


export { DirectBoardInput };
export default WelcomePage;
