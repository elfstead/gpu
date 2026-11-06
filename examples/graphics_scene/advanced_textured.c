/* Explicit consumer profile selection, never automatic backend emulation. */
static unsigned scene_advanced_mode;
#define SCENE_ADVANCED_RASTER
#define main textured_main
#include "textured.c"
#undef main
int main(int argc,char **argv) {
    if(argc!=5 || strlen(argv[1])!=1 || argv[1][0]<'0' || argv[1][0]>'4')return 1;
    scene_advanced_mode=(unsigned)(argv[1][0]-'0');
    printf("ADVANCED_PROFILE {\"profile\":%u,\"tessellation\":%s,\"geometry\":%s,\"continuation\":%s}\n",scene_advanced_mode,
        scene_advanced_mode>=2?"true":"false",scene_advanced_mode==1||scene_advanced_mode>=3?"true":"false",scene_advanced_mode==4?"true":"false");
    return textured_main(argc-1,argv+1);
}
