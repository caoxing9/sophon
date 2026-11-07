import math, random, sys
random.seed(11)
R=236; C=512
def streams(n, lmin, lmax, wmin, wmax, amin, amax):
    o=[]
    for i in range(n):
        t=random.uniform(0,2*math.pi)
        # brighter, longer streamers along a tilted axis (like a real corona)
        bias=abs(math.cos(t-math.radians(-28)))**2
        L=random.uniform(lmin,lmax)*(0.55+0.75*bias)
        r0=R-6; x0,y0=C+r0*math.cos(t),C+r0*math.sin(t)
        x1,y1=C+(R+L)*math.cos(t),C+(R+L)*math.sin(t)
        a=random.uniform(amin,amax)*(0.6+0.6*bias)
        o.append(f'<line x1="{x0:.1f}" y1="{y0:.1f}" x2="{x1:.1f}" y2="{y1:.1f}" stroke="url(#fade{i%3})" stroke-width="{random.uniform(wmin,wmax):.1f}" opacity="{a:.2f}"/>')
    return ''.join(o)
stars=''
for i in range(170):
    x,y=random.uniform(14,1010),random.uniform(14,1010)
    d=((x-C)**2+(y-C)**2)**.5
    if d<300: continue
    r=random.choice([1.2,1.5,1.8,2.2,2.6,3]); a=random.uniform(.35,.95)*min(1,(d-300)/120)
    stars+=f'<circle cx="{x:.0f}" cy="{y:.0f}" r="{r}" fill="#fff" opacity="{a:.2f}"/>'
for (x,y,r) in [(150,180,4),(860,150,3.6),(120,760,3.4),(905,820,4.2),(690,95,3)]:
    stars+=f'<circle cx="{x}" cy="{y}" r="{r*4}" fill="url(#twk)"/><circle cx="{x}" cy="{y}" r="{r}" fill="#fff"/>'
fades=''.join(f'<radialGradient id="fade{k}" gradientUnits="userSpaceOnUse" cx="512" cy="512" r="{r}"><stop offset="{R/r:.3f}" stop-color="{c}"/><stop offset="1" stop-color="{c}" stop-opacity="0"/></radialGradient>' for k,(r,c) in enumerate([(420,"#ffd6bd"),(470,"#ffb48a"),(390,"#fff1e6")]))
svg=f'''<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024"><defs>
<radialGradient id="bg" cx=".5" cy=".46" r=".78"><stop offset="0" stop-color="#1e1a30"/><stop offset=".5" stop-color="#0f0d1c"/><stop offset="1" stop-color="#040309"/></radialGradient>
<radialGradient id="twk"><stop offset="0" stop-color="#dfe6ff" stop-opacity=".7"/><stop offset="1" stop-color="#dfe6ff" stop-opacity="0"/></radialGradient>
<radialGradient id="neb" cx=".22" cy=".8" r=".5"><stop offset="0" stop-color="#3a2a5c" stop-opacity=".35"/><stop offset="1" stop-color="#3a2a5c" stop-opacity="0"/></radialGradient>
<radialGradient id="glow" gradientUnits="userSpaceOnUse" cx="512" cy="512" r="470"><stop offset=".48" stop-color="#ff9f70" stop-opacity=".55"/><stop offset=".62" stop-color="#e2754a" stop-opacity=".22"/><stop offset="1" stop-color="#b8462a" stop-opacity="0"/></radialGradient>
<radialGradient id="inner" gradientUnits="userSpaceOnUse" cx="512" cy="512" r="300"><stop offset=".76" stop-color="#fff3ea" stop-opacity="1"/><stop offset=".82" stop-color="#ffd0b2" stop-opacity=".75"/><stop offset=".92" stop-color="#ff9a68" stop-opacity=".2"/><stop offset="1" stop-color="#ff9a68" stop-opacity="0"/></radialGradient>
{fades}
<radialGradient id="disc" gradientUnits="userSpaceOnUse" cx="452" cy="440" r="300"><stop offset="0" stop-color="#1a1726"/><stop offset=".7" stop-color="#0a0912"/><stop offset="1" stop-color="#030207"/></radialGradient>
<radialGradient id="limb" gradientUnits="userSpaceOnUse" cx="512" cy="512" r="{R}"><stop offset=".9" stop-color="#ff9a68" stop-opacity="0"/><stop offset="1" stop-color="#ffb48a" stop-opacity=".22"/></radialGradient>
<linearGradient id="rim" x1=".15" y1="0" x2=".85" y2="1"><stop offset="0" stop-color="#fffaf5"/><stop offset=".55" stop-color="#ffd9c2" stop-opacity=".8"/><stop offset="1" stop-color="#ffb48a" stop-opacity=".45"/></linearGradient>
<filter id="b2" filterUnits="userSpaceOnUse" x="0" y="0" width="1024" height="1024"><feGaussianBlur stdDeviation="3"/></filter>
<filter id="b9" filterUnits="userSpaceOnUse" x="0" y="0" width="1024" height="1024"><feGaussianBlur stdDeviation="14"/></filter>
<filter id="b5" filterUnits="userSpaceOnUse" x="0" y="0" width="1024" height="1024"><feGaussianBlur stdDeviation="5"/></filter>
<filter id="grain" filterUnits="userSpaceOnUse" x="0" y="0" width="1024" height="1024"><feTurbulence type="fractalNoise" baseFrequency=".85" numOctaves="2" seed="4"/><feColorMatrix type="saturate" values="0"/><feComponentTransfer><feFuncA type="table" tableValues="0 .5"/></feComponentTransfer></filter>
</defs>
{{BODY}}
</svg>'''
# The icon in layers too, for the loading mark (the corona turns between a
# still sky and a still moon): `gen_icon.py out.svg [sky|corona|moon]`.
sky=f'''<rect width="1024" height="1024" fill="url(#bg)"/><rect width="1024" height="1024" fill="url(#neb)"/>{stars}'''
corona=f'''<circle cx="512" cy="512" r="470" fill="url(#glow)"/>
<g filter="url(#b9)" style="mix-blend-mode:screen">{streams(22,70,200,14,30,.22,.45)}</g>
<g filter="url(#b2)" style="mix-blend-mode:screen">{streams(150,20,150,1.4,3.2,.12,.42)}</g>
<circle cx="512" cy="512" r="300" fill="url(#inner)"/>
<circle cx="512" cy="512" r="{R+3}" fill="none" stroke="#fff6ef" stroke-width="10" filter="url(#b5)" opacity=".9"/>'''
moon=f'''<circle cx="512" cy="512" r="{R}" fill="url(#disc)"/>
<circle cx="512" cy="512" r="{R}" fill="url(#limb)"/>
<circle cx="512" cy="512" r="{R-1}" fill="none" stroke="url(#rim)" stroke-width="2.5"/>'''
grain='''<rect width="1024" height="1024" filter="url(#grain)" opacity=".12" style="mix-blend-mode:overlay"/>'''
layer=sys.argv[2] if len(sys.argv)>2 else "all"
body={"all":sky+"\n"+corona+"\n"+moon+"\n"+grain,"sky":sky+"\n"+grain,"corona":corona,"moon":moon}[layer]
svg=svg.replace("{BODY}",body)
open(sys.argv[1],'w').write(svg)
