import numpy as np, sys
sys.argv=[sys.argv[0]]
exec(open(__import__('os').path.join(__import__('os').path.dirname(__file__), 'qs.py')).read().split('# cantilever')[0])
# tilted strand pointing down at angle th below horizontal, root moves down by dz after tip touches
for th_deg in [20,45]:
    th=np.radians(th_deg); d=np.array([np.cos(th),0,-np.sin(th)])
    s=QS(root_dir=d); L=30.
    tip0=s.x[-1,2]
    for dz in [0.15,0.3]:
        s=QS(root_dir=d); s.root=np.array([0,0,-tip0-0.0]); s.x=s.rest_shape()
        # floor at z=0; lower root by dz
        s.root[2]-=dz; s.x=s.x+np.array([0,0,-dz])
        for it in range(8): mu=s.iterate(floor=np.zeros(V))
        N=mu.sum()
        exact=3*s.EI0*(dz*PX_M)/((L*PX_M)**3*np.cos(th)**2)
        print(f"th={th_deg} dz={dz}: N/exact={N/exact:.3f} minz={s.x[:,2].min():.2e} touching={np.nonzero(mu)[0]}")
# upright buckling: vertical strand with small curl, press progressively
curl=np.zeros((SEG,3)); curl[1:,1]=0.02
s=QS(root_dir=(0,0,-1),curl=curl)
s.root=np.array([0,0,30.]); s.x=s.rest_shape()
for step in range(60):
    s.root[2]-=0.25; s.x=s.x+np.array([0,0,-0.25])
    for it in range(2): mu=s.iterate(floor=np.zeros(V))
    if step%10==9:
        lens=np.linalg.norm(np.diff(s.x,axis=0),axis=1)
        print(f"press {30-s.root[2]-30+30-30+ (30-s.root[2]):.2f}: tip x={s.x[-1,0]:.2f} minz={s.x[:,2].min():.3f} N={mu.sum()*1e3:.3f}mN finite={np.isfinite(s.x).all()} lenerr={abs(lens-s.l).max():.1e}")
