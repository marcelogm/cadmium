import numpy as np, sys
sys.path.insert(0, __import__("os").path.dirname(__file__)); from strand import *
np.seterr(all='ignore')
def force_cant(h,ms,T=1.0,iters=1):
    s=Strand(); s.w/=ms; s.w[0]=0
    L=30.; F=3*s.EI0*1.0/L**3
    f=np.zeros((V,3)); f[-1,2]=-F
    hist=[]
    for k in range(int(T/h)):
        # iterations: repeat sweep (lambda reset each since 1 sweep form)
        hp=s.hprev or h; v=(s.x-s.xp)/hp; s.xp=s.x.copy()
        s.x=s.x+v*h+h*h*s.w[:,None]*f; s.x[0]=s.xp[0]
        for it in range(iters):
            for i in range(SEG): s.stretch(i); s.bend(i,h)
        s.hprev=h
        hist.append(-s.x[-1,2])
    hist=np.array(hist); t=np.arange(len(hist))*h
    final=hist[-1]
    ok=np.abs(hist-final)<0.05*abs(final)+1e-9
    # settle time: first time after which always ok
    idx=len(ok)-np.argmin(ok[::-1]) if not ok.all() else 0
    return final, t[min(idx,len(t)-1)]
for ms in [1e2,1e3,1e4,1e5]:
  for h in [1e-3,5e-4]:
    d,ts=force_cant(h,ms)
    print(f"mass x{ms:g} h={h}: defl/exact={d:.3f} settle={ts*1000:.0f}ms")
