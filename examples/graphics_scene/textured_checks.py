"""Shared acceptance for in-tree and independent SDK textured consumers."""
import math
import struct
import oracle
import textured_oracle as texture


def require(value,message):
    if not value:raise ValueError(message)


def check_output(folder,width,height):
    rows=[]
    for mode in range(len(texture.CASES)):
        for frame in range(3):
            prefix=folder/f'mode-{mode}-frame-{frame}'
            images=prefix.with_suffix('.images').read_bytes()
            require(prefix.with_suffix('.geometry').read_bytes()==oracle.geometry(frame==1,False),
                    'textured geometry/indirect/guard mismatch')
            expected=texture.GUARD+texture.texture(frame==1)+bytes([0xa5])*108
            require(prefix.with_suffix('.texture').read_bytes()==expected,'texture upload/guard mismatch')
            result=texture.check(images,width,height,int(frame==1),mode)
            if frame==2:
                require(images==(folder/f'mode-{mode}-frame-0.images').read_bytes(),'A/B/A image repeat mismatch')
            if mode==2:
                require(images==(folder/f'mode-1-frame-{frame}.images').read_bytes(),'view-relative mip mismatch')
            rows.append(dict(mode=mode,frame=frame,phase=int(frame==1),**result))
    return rows


def compare_outputs(native,public,width,height):
    """All pixels, including raster/filter tie bands omitted by the CPU oracle."""
    pixels=width*height;size=pixels*4;maximum=0;exact=True
    for mode in range(6):
        for frame in range(3):
            stem=f'mode-{mode}-frame-{frame}'
            for suffix in ('geometry','texture'):
                require((native/(stem+'.'+suffix)).read_bytes()==(public/(stem+'.'+suffix)).read_bytes(),
                        'native/public '+suffix+' mismatch')
            a=(native/(stem+'.images')).read_bytes();b=(public/(stem+'.images')).read_bytes()
            require(len(a)==len(b)==2*size+256,'native/public image extent mismatch')
            require(a[:64]==b[:64]==texture.GUARD and a[64+size:192+size]==b[64+size:192+size]==texture.GUARD*2
                    and a[-64:]==b[-64:]==texture.GUARD,'native/public guard mismatch')
            delta=max(abs(x-y) for x,y in zip(a[64:64+size],b[64:64+size]));maximum=max(maximum,delta)
            require(delta<=2,'native/public color mismatch')
            depths=zip(struct.iter_unpack('<f',a[192+size:-64]),struct.iter_unpack('<f',b[192+size:-64]))
            require(all(math.isfinite(x[0]) and math.isfinite(y[0]) and abs(x[0]-y[0])<=1e-6
                        for x,y in depths),'native/public depth mismatch')
            exact=exact and a==b
    return dict(frames=18,pixels=18*pixels,max_color_byte_delta=maximum,exact=exact)
