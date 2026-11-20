import numpy as np, sys
sys.path.insert(0, __import__("os").path.dirname(__file__)); from strand import *
np.seterr(all='ignore')
for pert in [0.0,0.01,0.3]:
    s=Strand(); s.x[-1,2]+=pert; s.xp=s.x.copy()
    out=[]
    for k in range(200):
        s.step(1e-3,np.zeros((V,3)),flip=k%2==1)
        if k in (0,1,2,5,20,199): out.append(f"{k}:{s.x[-1,2]:.3g}")
    print(pert,out)
