import numpy as np
exec(open(__import__('os').path.join(__import__('os').path.dirname(__file__), 'qs.py')).read().split('# cantilever')[0])
def head(d_um, R_mm=1.5, L_mm=12.0, E=3.4e9, alt_deg=60):
    d=d_um*1e-6; R=R_mm*1e-3
    n=4*0.7*R**2/d**2
    L=L_mm*1e-3/PX_M
    th=np.radians(alt_deg); dirn=np.array([np.cos(th),0,-np.sin(th)])
    curl=np.zeros((SEG,3)); curl[1:,1]=0.01
    s=QS(L=L,E=E,d=d,n=n,root_dir=dirn,curl=curl)
    s.root=np.array([0,0,-s.x[-1,2]]); s.x=s.rest_shape()
    for k in range(4000):
        s.root[2]-=L/800; s.x=s.x+np.array([0,0,-L/800])
        mu=s.iterate(floor=np.zeros(V)); mu=s.iterate(floor=np.zeros(V))
        e=np.diff(s.x,axis=0); a=np.degrees(np.arccos(np.clip(e[0]@e[-1]/np.linalg.norm(e[0])/np.linalg.norm(e[-1]),-1,1)))
        if a>=90 or s.root[2]<0.05*L: break
    return n,a,mu.sum(),s.root[2]/L
for d in [50,60,75,100]:
    n,a,N,h=head(d); print(f"d={d}um hairs={n:.0f} bend={a:.1f}deg N={N:.3f} N, root height/L={h:.2f}")
