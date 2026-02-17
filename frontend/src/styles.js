import { priorityColor } from './utils';

// ---- iOS Safari zoom reset on app resume ----
if (typeof document !== 'undefined') {
  const isIOS = /iPad|iPhone|iPod/.test(navigator.userAgent) ||
    (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);
  if (isIOS) {
    const viewport = document.querySelector('meta[name="viewport"]');
    if (viewport) {
      viewport.content = 'width=device-width, initial-scale=1.0, maximum-scale=1.0';
    }
  }
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
      const viewport = document.querySelector('meta[name="viewport"]');
      if (viewport) {
        const original = viewport.content;
        viewport.content = 'width=device-width, initial-scale=1.0, maximum-scale=1.0';
        setTimeout(() => { viewport.content = original; }, 100);
      }
    }
  });
}

const styles = {
  app: (mobile) => (mobile
    ? { minHeight: '100dvh', display: 'block', overflowX: 'hidden' }
    : { height: '100dvh', display: 'flex', flexDirection: 'column' }
  ),
  header: (mobile) => ({
    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
    padding: mobile ? '8px 10px' : '12px 20px', background: '#1e293b',
    borderBottom: '1px solid #334155',
    minHeight: mobile ? '40px' : '48px', overflow: 'visible',
    gap: '8px',
  }),
  logo: { fontSize: '1.2rem', fontWeight: 700, color: '#f1f5f9', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '8px', flexShrink: 0 },
  logoImg: { width: '24px', height: '24px' },
  headerRight: { display: 'flex', alignItems: 'center', gap: '6px', fontSize: '0.85rem', flexShrink: 1, minWidth: 0 },
  menuBtn: {
    background: '#1e293b', border: '1px solid #334155', color: '#94a3b8',
    padding: '7px', borderRadius: '6px', cursor: 'pointer',
    lineHeight: 0, transition: 'background 0.15s, border-color 0.15s, color 0.15s',
    display: 'flex', alignItems: 'center', justifyContent: 'center',
    width: '34px', height: '34px',
  },
  modeBadge: (canEdit) => ({
    fontSize: '0.7rem', fontWeight: 600,
    padding: '3px 8px', borderRadius: '12px',
    background: canEdit ? '#22c55e22' : '#64748b22',
    color: canEdit ? '#22c55e' : '#94a3b8',
    border: `1px solid ${canEdit ? '#22c55e44' : '#64748b44'}`,
    whiteSpace: 'nowrap',
  }),
  main: (mobile) => ({
    flex: mobile ? undefined : 1,
    display: 'flex',
    flexDirection: mobile ? 'column' : 'row',
    overflow: mobile ? 'visible' : 'hidden',
    position: 'relative',
  }),
  sidebar: (mobile, open) => ({
    ...(mobile ? {
      position: 'fixed', top: 0, left: 0, bottom: 0,
      width: '280px', maxWidth: '85vw', zIndex: 200,
      transform: open ? 'translateX(0)' : 'translateX(-100%)',
      transition: 'transform 0.2s ease',
    } : {
      width: '240px', minWidth: '240px',
    }),
    background: '#1e293b',
    borderRight: '1px solid #334155', display: 'flex', flexDirection: 'column',
    overflow: 'auto',
  }),
  sidebarOverlay: {
    position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.5)', zIndex: 199,
  },
  sidebarHeader: {
    padding: '12px 16px', fontSize: '0.75rem', fontWeight: 600, color: '#94a3b8',
    textTransform: 'uppercase', letterSpacing: '0.05em',
    display: 'flex', justifyContent: 'space-between', alignItems: 'center',
  },
  boardItem: (active) => ({
    padding: '10px 16px', cursor: 'pointer', fontSize: '0.9rem',
    background: active ? '#334155' : 'transparent',
    color: active ? '#f1f5f9' : '#94a3b8',
    borderLeft: active ? '3px solid #6366f1' : '3px solid transparent',
    display: 'flex', justifyContent: 'space-between', alignItems: 'center',
  }),
  archivedBadge: {
    fontSize: '0.65rem', background: '#475569', color: '#94a3b8',
    padding: '1px 5px', borderRadius: '3px',
  },
  boardContent: (mobile) => ({
    flex: mobile ? undefined : 1,
    display: 'flex',
    flexDirection: 'column',
    overflow: mobile ? 'visible' : 'hidden',
    position: 'relative',
  }),
  boardHeader: (mobile) => ({
    padding: mobile ? '12px' : '16px 20px',
    display: 'flex', alignItems: mobile ? 'flex-start' : 'center',
    justifyContent: 'space-between',
    flexDirection: mobile ? 'column' : 'row', gap: mobile ? '8px' : '0',
  }),
  boardTitle: (mobile) => ({
    fontSize: mobile ? '1.1rem' : '1.3rem', fontWeight: 700, color: '#f1f5f9',
  }),
  columnsContainer: (mobile) => ({
    flex: mobile ? undefined : 1,
    display: 'flex',
    flexDirection: mobile ? 'column' : 'row',
    gap: mobile ? '12px' : '16px',
    padding: mobile ? '12px' : '16px 20px',
    overflowX: mobile ? 'hidden' : 'auto',
    overflowY: mobile ? 'visible' : 'hidden',
    alignItems: mobile ? 'stretch' : 'stretch',
    minHeight: 0,
  }),
  column: (isDragOver, mobile) => ({
    ...(mobile ? {
      flex: 'none', width: '100%',
    } : {
      minWidth: '280px', maxWidth: '320px', flex: '0 0 280px',
    }),
    background: isDragOver ? '#1e293b' : '#1a2332', borderRadius: '8px',
    border: isDragOver ? '2px dashed #6366f1' : '1px solid #334155',
    display: 'flex', flexDirection: 'column',
    maxHeight: mobile ? 'none' : '100%',
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
  taskList: (mobile) => ({
    flex: mobile ? 'none' : 1,
    overflow: mobile ? 'visible' : 'auto',
    padding: '8px',
  }),
  card: (isDragging, priority) => ({
    background: isDragging ? '#334155' : '#0f172a',
    border: `1px solid ${priorityColor(priority)}33`,
    borderLeft: `3px solid ${priorityColor(priority)}`,
    borderRadius: '6px', padding: '10px 12px', marginBottom: '8px',
    cursor: isDragging ? 'grabbing' : 'pointer',
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
  btn: (variant = 'primary', mobile) => ({
    background: variant === 'primary' ? '#6366f1' : variant === 'danger' ? '#ef4444' : '#334155',
    color: '#fff', border: 'none', outline: 'none',
    padding: mobile ? '8px 14px' : '6px 12px',
    borderRadius: '4px', cursor: 'pointer',
    fontSize: mobile ? '0.85rem' : '0.8rem', fontWeight: 500,
    whiteSpace: 'nowrap',
    height: '32px', lineHeight: '1', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
    boxSizing: 'border-box',
  }),
  btnSmall: {
    background: '#334155', border: '1px solid #475569', color: '#cbd5e1',
    padding: '3px 8px', borderRadius: '4px', cursor: 'pointer', fontSize: '0.75rem',
    height: '32px', lineHeight: '1', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
    boxSizing: 'border-box', outline: 'none',
  },
  btnClose: {
    background: 'transparent', border: '1px solid #334155', color: '#94a3b8',
    width: '32px', height: '32px', borderRadius: '4px', cursor: 'pointer',
    fontSize: '1rem', lineHeight: 1, padding: 0,
    display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
    boxSizing: 'border-box', flexShrink: 0,
  },
  btnIcon: {
    background: '#334155', border: '1px solid #475569', color: '#cbd5e1',
    width: '32px', height: '32px', borderRadius: '4px', cursor: 'pointer',
    fontSize: '0.8rem', lineHeight: 1, padding: 0,
    display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
    boxSizing: 'border-box', flexShrink: 0,
  },
  modal: (mobile) => ({
    position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.6)',
    display: 'flex', alignItems: mobile ? 'stretch' : 'flex-start', justifyContent: 'center', zIndex: 1100,
    padding: mobile ? '0' : '12px',
    paddingTop: mobile ? '0' : '4vh',
  }),
  modalContent: (mobile) => ({
    background: '#1e293b', border: mobile ? 'none' : '1px solid #334155', borderRadius: mobile ? '0' : '8px',
    padding: mobile ? '16px' : '24px', paddingBottom: mobile ? '24px' : '24px',
    width: mobile ? '100%' : '480px', maxWidth: '100%',
    maxHeight: mobile ? '100dvh' : '90vh', height: mobile ? '100dvh' : 'auto', overflow: 'auto',
  }),
  modalContentWide: (mobile) => ({
    background: '#1e293b', border: mobile ? 'none' : '1px solid #334155', borderRadius: mobile ? '0' : '8px',
    padding: mobile ? '16px' : '24px', paddingBottom: mobile ? '24px' : '24px',
    width: mobile ? '100%' : '680px', maxWidth: '100%',
    maxHeight: mobile ? '100dvh' : '90vh', height: mobile ? '100dvh' : 'auto', overflow: 'auto',
  }),
  input: {
    width: '100%', background: '#0f172a', border: '1px solid #334155', color: '#e2e8f0',
    padding: '10px', borderRadius: '4px', fontSize: '16px', marginBottom: '10px',
    boxSizing: 'border-box',
  },
  textarea: {
    width: '100%', background: '#0f172a', border: '1px solid #334155', color: '#e2e8f0',
    padding: '10px', borderRadius: '4px', fontSize: '16px', minHeight: '140px',
    resize: 'vertical', marginBottom: '10px', fontFamily: 'inherit',
    boxSizing: 'border-box',
  },
  select: {
    background: '#0f172a', border: '1px solid #334155', color: '#e2e8f0',
    padding: '8px', borderRadius: '4px', fontSize: '16px', marginBottom: '10px',
    flex: 1, boxSizing: 'border-box',
  },
  empty: {
    textAlign: 'center', color: '#475569', padding: '40px 20px', fontSize: '0.9rem',
  },
  searchBar: (mobile) => ({
    display: 'flex', gap: '8px',
    padding: mobile ? '0 12px' : '0 20px',
    paddingBottom: '0',
  }),
  urlBox: {
    background: '#0f172a', border: '1px solid #334155', borderRadius: '4px',
    padding: '10px 12px', fontSize: '0.78rem', color: '#94a3b8',
    fontFamily: 'monospace', wordBreak: 'break-all', marginBottom: '10px',
    display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '8px',
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
    padding: '8px 10px', borderRadius: '4px', fontSize: '16px', flex: 1,
    minWidth: 0, boxSizing: 'border-box',
  },
};

export default styles;
