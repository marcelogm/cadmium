import numpy as np
np.seterr(all='ignore')
PX_M=4e-5; V=8; SEG=7
def kb(e0,e1): return 2*np.cross(e0,e1)/(np.linalg.norm(e0)*np.linalg.norm(e1)+e0@e1)
def skew(t): return np.array([[0,-t[2],t[1]],[t[2],0,-t[0]],[-t[1],t[0],0]])
class S:
    def __init__(s,L=30.,E=4e9,d=60e-6,rho=1300.,n=3.,taper=1.0,ms=1.0):
        s.l=L/SEG; s.d0=np.array([1.,0,0])
        s.x=np.array([s.d0*s.l*i for i in range(V)]); s.xp=s.x.copy()
        r=d/2*(1-(1-taper)*np.linspace(0,1,V))
        EI=n*E*np.pi*r**4/4/PX_M**3; m=ms*n*rho*np.pi*r**2*s.l*PX_M
        s.w=1/m; s.w[0]=0; lb=np.full(V,s.l); lb[0]=s.l/2
        s.alpha=lb/EI; s.EI0=EI[0]; s.hp=None
    def solve(s,h,contact=None):
        # constraints: stretch j (1 row), bend i (3 rows)
        rows=[];C=[];A=[]
        for j in range(SEG):
            e=s.x[j+1]-s.x[j]; L=np.linalg.norm(e); g=e/L
            J=np.zeros(3*V); J[3*j:3*j+3]=-g; J[3*j+3:3*j+6]=g
            rows.append(J); C.append(L-s.l); A.append(0.0)
        for i in range(SEG):
            prev=s.x[0]-s.d0*s.l if i==0 else s.x[i-1]
            e0=s.x[i]-prev; e1=s.x[i+1]-s.x[i]; l0=np.linalg.norm(e0); l1=np.linalg.norm(e1)
            c=kb(e0,e1); t=e0/l0+e1/l1; t/=np.linalg.norm(t); K=skew(t)
            Jb=np.zeros((3,3*V))
            if i>0: Jb[:,3*(i-1):3*i]=K/l0
            Jb[:,3*i:3*i+3]=-K*(1/l0+1/l1); Jb[:,3*i+3:3*i+6]=K/l1
            for k in range(3): rows.append(Jb[k]); C.append(c[k]); A.append(s.alpha[i]/h/h)
        J=np.array(rows); C=np.array(C); A=np.array(A)
        W=np.repeat(s.w,3)
        M=(J*W)@J.T+np.diag(A)+1e-12*np.eye(len(C))
        dl=np.linalg.solve(M,-C)
        s.x=s.x+(W*(J.T@dl)).reshape(V,3)
    def step(s,h,f,iters=1,contact=None):
        hp=s.hp or h; v=(s.x-s.xp)/hp; s.xp=s.x.copy()
        s.x=s.x+v*h+h*h*s.w[:,None]*f; s.x[0]=s.xp[0]
        for _ in range(iters): s.solve(h)
        if contact is not None: s.x[-1,2]=contact
        s.hp=h
def force_test(h,T=0.2,iters=1):
    s=S(); L=30.; F=3*s.EI0*1.0/L**3; f=np.zeros((V,3)); f[-1,2]=-F
    hist=[]
    for k in range(int(T/h)): s.step(h,f,iters); hist.append(-s.x[-1,2])
    return hist[-1], hist[int(len(hist)*0.5)], hist[2]
for h in [1e-3,5e-4,2e-3,5e-3]:
  for it in [1,2]:
    print("force h",h,"iters",it,["%.4f"%v for v in force_test(h,iters=it)], "exact 1.0")
def disp_test(h,T=0.2):
    s=S(); L=30.; delta=3.0; Fs=[]
    for k in range(int(T/h)):
        tgt=-delta*min(1,k/(T/h/4)); s.step(h,np.zeros((V,3)))
        dz=tgt-s.x[-1,2]; s.x[-1,2]=tgt; Fs.append(-dz/s.w[-1]/h/h)
    xs=s.x[:,0]; shape=delta*xs**2*(3*L-xs)/(2*L**3)
    return np.mean(Fs[-20:])/(3*s.EI0*delta/L**3), np.abs(-s.x[:,2]-shape).max()
for h in [1e-3,5e-4]: print("disp h",h,disp_test(h))
