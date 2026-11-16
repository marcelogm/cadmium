import numpy as np
PX_M=4e-5; V=8; SEG=7; Z=np.array([0,0,1.])
def rot(v,w):  # rotate v by rotation vector w (Rodrigues)
    a=np.linalg.norm(w)
    if a<1e-12: return v+np.cross(w,v)
    k=w/a; return v*np.cos(a)+np.cross(k,v)*np.sin(a)+k*(k@v)*(1-np.cos(a))
def rotvec(a,b): # minimal rotation vector taking unit a to unit b
    c=np.cross(a,b); s=np.linalg.norm(c); d=a@b
    if s<1e-12: return np.zeros(3)
    return c/s*np.arctan2(s,d)
class QS:
    def __init__(s,L=30.,E=4e9,d=60e-6,n=3.,taper=1.0,root_dir=(1,0,0),curl=None):
        s.l=L/SEG; s.d0=np.array(root_dir,float)/np.linalg.norm(root_dir)
        r=d/2*(1-(1-taper)*np.linspace(0,1,V))
        EI=n*E*np.pi*r**4/4  # SI
        lb=np.full(SEG,s.l*PX_M); lb[0]/=2
        s.alpha=lb/EI[:SEG]; s.EI0=EI[0]
        s.k0=np.zeros((SEG,3)) if curl is None else curl
        s.root=np.zeros(3)
        s.x=s.rest_shape()
    def rest_shape(s):
        x=[s.root.copy()]; e=s.d0.copy()
        for i in range(SEG):
            e=rot(e,s.k0[i]); x.append(x[-1]+e*s.l)
        return np.array(x)
    def hinges(s):
        e=np.diff(s.x,axis=0); e/= np.linalg.norm(e,axis=1)[:,None]
        prev=np.vstack([s.d0,e[:-1]])
        return np.array([rotvec(prev[i],e[i]) for i in range(SEG)]), e
    def iterate(s,floor=None,F=None,cap=0.3,eps=1e-12):
        k,e=s.hinges()
        dth=s.k0-k          # unconstrained: back to rest (rest curvature approx in world)
        D=s.alpha
        # external tip force
        if F is not None:
            for i in range(SEG):
                r=(s.x[-1]-s.x[i])*PX_M
                dth[i]+=D[i]*np.cross(r,F)
        mu=np.zeros(V)
        if floor is not None:
            def predz(dth):
                z=s.x[:,2].copy()
                for j in range(1,V):
                    for i in range(j): z[j]+= (np.cross(dth[i],(s.x[j]-s.x[i])))[2]
                return z
            active=[j for j in range(1,V) if predz(dth)[j]<floor[j]-1e-9]
            for _ in range(8):
                if not active: break
                # A_j,i = (r_ji x Z) in px per rad ; force mu in N, moment = r(m) x (mu Z)
                Am=np.zeros((len(active),SEG,3)); 
                for a,j in enumerate(active):
                    for i in range(j): Am[a,i]=np.cross(s.x[j]-s.x[i],Z)
                G=np.zeros((len(active),len(active)))
                for a in range(len(active)):
                    for b in range(len(active)):
                        jb=active[b]
                        G[a,b]=sum(D[i]*(Am[a,i]@np.cross((s.x[jb]-s.x[i])*PX_M,Z)) for i in range(SEG))
                z0=predz(dth)
                rhs=np.array([floor[j]-z0[j] for j in active])
                m=np.linalg.solve(G+eps*np.eye(len(active))*np.trace(G),rhs)
                if (m>=0).all():
                    trial=dth.copy()
                    for a,j in enumerate(active):
                        for i in range(j): trial[i]+=D[i]*np.cross((s.x[j]-s.x[i])*PX_M,Z)*m[a]
                    zt=predz(trial)
                    extra=[j for j in range(1,V) if j not in active and zt[j]<floor[j]-1e-9]
                    if not extra:
                        dth=trial
                        for a,j in enumerate(active): mu[j]=m[a]
                        break
                    active=sorted(active+extra)
                    continue
                keep=[j for a,j in enumerate(active) if m[a]>0]
                active=keep
        # cap and apply exactly
        for i in range(SEG):
            a=np.linalg.norm(dth[i])
            if a>cap: dth[i]*=cap/a
        e_new=[]; 
        for i in range(SEG):
            # rotate hinge i: all downstream edges rotate by dth[i]
            pass
        e=list(e)
        for i in range(SEG):
            for j in range(i,SEG): e[j]=rot(e[j],dth[i])
        x=[s.x[0]]
        for i in range(SEG): x.append(x[-1]+e[i]/np.linalg.norm(e[i])*s.l)
        s.x=np.array(x)
        return mu
# cantilever force control
for Fmul in [0.2,1.0]:
    s=QS(); L=30*PX_M
    F_N=3*s.EI0*(Fmul*1.0*PX_M)/L**3
    for it in range(6):
        s.iterate(F=np.array([0,0,-F_N]))
        print(f"F x{Fmul} it{it}: defl/exact={-s.x[-1,2]/(Fmul):.4f}")
