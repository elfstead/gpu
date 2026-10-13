"""Independent M4 texture/filter/depth/blend oracle; no GPU or shader imports."""
import math
import struct

GUARD = bytes([0xa5]) * 64
# (first mip in view, explicit view-relative LOD, linear filtering, inset viewport)
CASES = ((0, 0.0, False, False), (0, 1.0, False, False),
         (1, 0.0, False, False), (0, 0.5, True, False),
         (0, 0.0, False, True), (1, 0.5, True, True))


def texel(mip, x, y, phase):
    """8x8,4x4,2x2,1x1 RGBA8 mip chain, changed in the B frame."""
    return ((32 + 13*x + 7*y + 23*mip + 17*phase) % 192,
            (48 + 5*x + 19*y + 31*mip + 11*phase) % 192,
            (64 + 17*x + 3*y + 29*mip + 13*phase) % 192, 255)


def texture(phase):
    return b''.join(bytes(texel(mip, x, y, phase)) for mip in range(4)
                    for y in range(8 >> mip) for x in range(8 >> mip))


def sample_level(mip, u, v, phase, linear):
    width = 8 >> mip
    def pixel(x, y):
        return texel(mip, min(width-1, max(0, x)), min(width-1, max(0, y)), phase)[:3]
    if not linear:
        return pixel(math.floor(u*width), math.floor(v*width))
    x, y = u*width-.5, v*width-.5
    left, top = math.floor(x), math.floor(y)
    fx, fy = x-left, y-top
    taps = (pixel(left, top), pixel(left+1, top), pixel(left, top+1), pixel(left+1, top+1))
    return tuple((a*(1-fx)+b*fx)*(1-fy)+(c*(1-fx)+d*fx)*fy for a,b,c,d in zip(*taps))


def sample(first_mip, lod, u, v, phase, linear):
    level = min(3., max(float(first_mip), first_mip+lod))
    if not linear:
        return sample_level(math.floor(level+.5), u, v, phase, False)
    low = math.floor(level); fraction = level-low
    return tuple(a*(1-fraction)+b*fraction for a,b in zip(
        sample_level(low,u,v,phase,True), sample_level(min(3,low+1),u,v,phase,True)))


def viewport(width, height, inset):
    x, y = (width//8, height//8) if inset else (0, 0)
    return x, y, width-2*x, height-2*y


def scissor(width, height, inset, overlay=False):
    if overlay:
        return width//4, height//3, width//2, height//3
    if inset:
        return width//4, height//4, width//2, height//2
    return 0, 0, width, height


def inside(px, py, rect):
    x,y,w,h = rect
    return x <= px < x+w and y <= py < y+h


def expected(column, row, width, height, phase, mode):
    first,lod,linear,inset = CASES[mode]
    vx,vy,vw,vh = viewport(width,height,inset)
    x,y = 2*(column+.5-vx)/vw-1, 2*(row+.5-vy)/vh-1
    center = -.125 if phase == 0 else .125
    far = abs(x)<.75 and abs(y)<.75
    near = abs(x-center)<.25 and abs(y)<.375
    color,depth = (0.,0.,0.),1.
    if inside(column,row,scissor(width,height,inset)) and (near or far):
        u,v = ((x-center+.25)/.5,(y+.375)/.75) if near else ((x+.75)/1.5,(y+.75)/1.5)
        color = sample(first,lod,u,v,phase,linear)
        if near:color = (color[0]*.5,color[1],color[2]*.5)
        depth = .25 if near else .75
    # Overlay reuses the far quad, ignores depth and never writes it. It has its
    # own scissor; it can blend over clear pixels outside the opaque scissor.
    if far and inside(column,row,scissor(width,height,inset,True)):
        color = tuple(.5*c+.5*255*t for c,t in zip(color,(.25,.5,1.)))
    return (*color,255.),depth


def interior(column, row, width, height, phase, mode):
    first,lod,linear,inset = CASES[mode]
    vx,vy,vw,vh = viewport(width,height,inset)
    x,y = 2*(column+.5-vx)/vw-1, 2*(row+.5-vy)/vh-1
    center = -.125 if phase==0 else .125
    # Exclude one output-pixel band at polygon edges, not integer scissors.
    if (any(abs(x-edge)<=2/vw for edge in (-.75,.75,center-.25,center+.25)) or
        any(abs(y-edge)<=2/vh for edge in (-.75,.75,-.375,.375))):return False
    if linear:return True
    # Nearest-filter tie points depend on interpolator precision. Exclude only
    # a narrow UV band, not entire texels or all textured pixels.
    mip=first+math.floor(lod+.5); size=8>>mip
    for u,v in (((x+.75)/1.5,(y+.75)/1.5),((x-center+.25)/.5,(y+.375)/.75)):
        if 0<u<1 and 0<v<1 and any(abs(t*size-round(t*size))<1e-5 for t in (u,v)):
            return False
    return True


def check(images, width, height, phase, mode):
    if width<64 or height<64 or phase not in (0,1) or mode not in range(len(CASES)):
        raise ValueError('invalid textured scene case')
    size=width*height*4
    if len(images)!=2*size+256:raise ValueError('textured image extent mismatch')
    if images[:64]!=GUARD or images[64+size:192+size]!=GUARD*2 or images[-64:]!=GUARD:
        raise ValueError('textured image guard mismatch')
    checked=0
    for row in range(height):
        for column in range(width):
            if not interior(column,row,width,height,phase,mode):continue
            pixel=row*width+column
            color=images[64+pixel*4:68+pixel*4]
            depth=struct.unpack_from('<f',images,192+size+pixel*4)[0]
            want,z=expected(column,row,width,height,phase,mode)
            # RGBA8 sampling/interpolation, tint, attachment conversion and
            # blending may each round. Predeclared <=2 byte color tolerance.
            if any(abs(a-b)>2 for a,b in zip(color,want)) or not math.isfinite(depth) or abs(depth-z)>1e-6:
                raise ValueError(f'textured oracle mismatch mode={mode} phase={phase} pixel={column},{row}')
            checked+=1
    if checked<width*height*.85:raise ValueError('insufficient textured interior coverage')
    return dict(checked_pixels=checked,excluded_edge_pixels=width*height-checked)
