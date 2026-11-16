import numpy as np, sys
PX_M=4e-5
V=8; SEG=V-1
def kb(e0,e1):
    return 2*np.cross(e0,e1)/(np.linalg.norm(e0)*np.linalg.norm(e1)+e0@e1)
class Strand:
    def __init__(s,L=30.,E=4e9,d=60e-6,rho=1300.,n=3.,taper=1.0,root_dir=(1,0,0),damp=0.0):
        s.l=L/SEG
        s.x=np.array([np.array(root_dir,float)*s.l*i for i in range(V)])
        s.xp=s.x.copy(); s.d0=np.array(root_dir,float)
        sh=np.linspace(0,1,V); r=d/2*(1-(1-taper)*sh)
        EI=n*E*np.pi*r**4/4/PX_M**3      # kg px^3/s^2
        lm=s.l*PX_M
        m=n*rho*np.pi*r**2*lm
        s.w=1/m; s.w[0]=0
        lbar=np.full(V,s.l); lbar[0]=s.l/2
        s.alpha=lbar/EI   # vertex bends 0..V-2
        s.EI0=EI[0]; s.damp=damp
        s.hprev=None
    def step(s,h,f_ext,flip=False):
        hp=s.hprev or h
        v=(s.x-s.xp)/hp
        s.xp=s.x.copy()
        s.x=s.x+v*h+h*h*s.w[:,None]*f_ext
        s.x[0]=s.xp[0]
        order=range(SEG) if not flip else range(SEG-1,-1,-1)
        for i in order:
            # stretch segment i (i,i+1)
            s.stretch(i)
            # bend at vertex i
            s.bend(i,h)
        s.hprev=h
    def stretch(s,i):
        a,b=s.x[i],s.x[i+1]; wa,wb=s.w[i],s.w[i+1]
        e=b-a; L=np.linalg.norm(e); c=L-s.l; g=e/L
        dl=-c/(wa+wb)
        s.x[i]-=wa*dl*g; s.x[i+1]+=wb*dl*g
    def bend(s,i,h):
        if i==0: prev=s.x[0]-s.d0*s.l; wp=0.
        else: prev=s.x[i-1]; wp=s.w[i-1]
        e0=s.x[i]-prev; e1=s.x[i+1]-s.x[i]
        l0=np.linalg.norm(e0); l1=np.linalg.norm(e1)
        C=kb(e0,e1)
        t=e0/l0+e1/l1; t/=np.linalg.norm(t)
        a=np.array([1/l0,-(1/l0+1/l1),1/l1]); ws=np.array([wp,s.w[i],s.w[i+1]])
        sw=(ws*a*a).sum()
        at=s.alpha[i]/h/h
        dlam=-C/(sw+at)
        corr=np.cross(dlam,t)
        if i>0: s.x[i-1]+=wp*a[0]*corr
        s.x[i]+=ws[1]*a[1]*corr
        s.x[i+1]+=ws[2]*a[2]*corr
def cantilever(h, T=0.5, F=None):
    s=Strand()
    L=30.
    if F is None: F=3*s.EI0*1.0/L**3   # 1 px expected
    f=np.zeros((V,3)); f[-1,2]=-F
    n=int(T/h)
    for k in range(n): s.step(h,f,flip=k%2==1)
    exact=F*L**3/(3*s.EI0)
    return -s.x[-1,2], exact, s
for h in [1e-3,5e-4,2e-3,5e-3]:
    d,e,s=cantilever(h)
    print(f"h={h} defl={d:.4f} exact={e:.4f} err={(d-e)/e*100:.2f}% len={np.linalg.norm(np.diff(s.x,axis=0),axis=1).max():.4f}")
