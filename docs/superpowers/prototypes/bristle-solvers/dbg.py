import numpy as np, sys
sys.path.insert(0, __import__("os").path.dirname(__file__)); from strand import *
np.seterr(all='ignore')
for flipmode in [False,True]:
    s=Strand(); s.x[-1,2]+=0.01; s.xp=s.x.copy()
    for k in range(120):
        s.step(1e-3,np.zeros((V,3)),flip=(k%2==1) if flipmode else False)
        if k%10==0:
            v=np.abs(s.x-s.xp).max()
            lens=np.linalg.norm(np.diff(s.x,axis=0),axis=1)
            print(flipmode,k,f"maxdx={v:.3g} lenerr={np.abs(lens-s.l).max():.3g} tipz={s.x[-1,2]:.3g} tipy={s.x[-1,1]:.3g}")
