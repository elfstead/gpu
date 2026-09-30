"""Independent rectangle/occlusion oracle; not a triangle rasterizer."""
import math
import struct

GUARD = bytes([0xa5])*64
BLACK, RED, GREEN = bytes((0,0,0,255)), bytes((255,0,0,255)), bytes((0,255,0,255))


def expected(x, y, phase, mode):
    far = abs(x) < .75 and abs(y) < .75
    near = abs(x - (-.125 if phase == 0 else .125)) < .25 and abs(y) < .375
    if mode in (5, 9):
        return BLACK, 0. if mode == 5 else 1.
    if mode in (2, 3, 4, 8):
        color = RED if far else GREEN if near else BLACK
        depth = 1. if mode in (2, 3) else .75 if far else .25 if near and mode != 8 else 1.
        return color, depth
    color = (BLACK if mode == 7 else GREEN) if near else RED if far else BLACK
    return color, .25 if near else .75 if far else 1.


def interior(x, y, width, height, phase):
    center = -.125 if phase == 0 else .125
    # Conservative whole-line exclusion; no claim about edge rasterization rules.
    return (all(abs(x-edge) > 2/width for edge in (-.75,.75,center-.25,center+.25))
            and all(abs(y-edge) > 2/height for edge in (-.75,.75,-.375,.375)))


def geometry(phase, empty, index_bytes=4):
    # Explicit vertex table and native indirect layout, separate from image oracle.
    center = -.125 if phase == 0 else .125
    vertices = [(-.75,-.75,.75,1.),(.75,-.75,.75,1.),(.75,.75,.75,1.),(-.75,.75,.75,1.),
                (center-.25,-.375,.25,1.),(center+.25,-.375,.25,1.),
                (center+.25,.375,.25,1.),(center-.25,.375,.25,1.)]
    v = struct.pack('<32f', *(component for vertex in vertices for component in vertex))
    if index_bytes not in (2,4): raise ValueError('unsupported index width')
    poison=(1 << (8*index_bytes))-1
    indices = struct.pack('<8'+('H' if index_bytes==2 else 'I'), poison,poison,1,2,3,1,3,4)
    draws = b''.join(struct.pack('<IIIiI', 0 if empty else 6,1,2,base,0) for base in (-1,3))
    return GUARD+v+GUARD+GUARD+indices+bytes([0xa5])*(96-len(indices))+GUARD+draws+GUARD


def check(images, mesh, width, height, phase, mode, index_bytes=4):
    size=width*height*4
    if len(images)!=2*size+256 or mesh!=geometry(phase,mode==9,index_bytes):
        raise ValueError('image size or computed geometry/index/indirect/guard mismatch')
    if images[:64]!=GUARD or images[64+size:192+size]!=GUARD*2 or images[-64:]!=GUARD:
        raise ValueError('image readback guard mismatch')
    color=images[64:64+size]
    depth=struct.unpack(f'<{width*height}f',images[192+size:192+2*size])
    checked=0
    for row in range(height):
        y=2*(row+.5)/height-1
        for column in range(width):
            x=2*(column+.5)/width-1
            if not interior(x,y,width,height,phase): continue
            index=row*width+column
            want,z=expected(x,y,phase,mode)
            if color[4*index:4*index+4]!=want or not math.isfinite(depth[index]) or abs(depth[index]-z)>1e-6:
                raise ValueError(f'oracle mismatch mode={mode} phase={phase} pixel={column},{row}: '
                                 f'color={list(color[4*index:4*index+4])} depth={depth[index]} expected={list(want)},{z}')
            checked+=1
    if checked < width*height*.9: raise ValueError('insufficient interior coverage')
    return dict(checked_pixels=checked, excluded_edge_pixels=width*height-checked)
