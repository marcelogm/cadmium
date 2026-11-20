import numpy as np, sys
sys.path.insert(0, __import__("os").path.dirname(__file__)); from strand import *
def run(h,T,mass_scale=1.0,delta=3.0,taper=1.0):
    s=Strand(taper=taper); s.w*=1/mass_scale; s.w[0]=0
    L=30.; force_sum=0; cnt=0
    n=int(T/h)
    for k in range(n):
        # ramp target over first half
        tgt=-delta*min(1,k/(n/2))
        s.step(h,np.zeros((V,3)),flip=k%2==1)
        dz=tgt-s.x[-1,2]
        s.x[-1,2]=tgt
        if k>n*0.8: force_sum+=-dz/s.w[-1]/h/h; cnt+=1
    F=force_sum/cnt
    exact=3*s.EI0*delta/L**3
    xs=s.x[:,0]; shape=delta*xs**2*(3*L-xs)/(2*L**3)
    return F/exact, np.abs(-s.x[:,2]-shape).max()
for ms in [1,1e2,1e4,1e6]:
  for h in [1e-3,5e-4]:
    r,e=run(h,1.0,ms)
    print(f"mass x{ms:g} h={h}: force/exact={r:.3f} shape err={e:.3f}px")
