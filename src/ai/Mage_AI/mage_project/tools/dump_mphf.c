#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>

int main(int argc,char** argv){
    if(argc<2){fprintf(stderr,"Usage: %s <mphf.bin>\n",argv[0]); return 2;}
    FILE* f = fopen(argv[1],"rb"); if(!f){perror("fopen"); return 1;}
    size_t num_keys=0, table_size=0; uint64_t seed=0;
    if(fread(&num_keys,sizeof(size_t),1,f)!=1){fprintf(stderr,"read fail\n"); return 1;}
    if(fread(&table_size,sizeof(size_t),1,f)!=1){fprintf(stderr,"read fail\n"); return 1;}
    if(fread(&seed,sizeof(uint64_t),1,f)!=1){fprintf(stderr,"read fail\n"); return 1;}
    printf("num_keys=%zu table_size=%zu seed=%llu\n",num_keys,table_size,(unsigned long long)seed);
    for(size_t i=0;i<table_size;++i){
        int32_t val=-1; if(fread(&val,sizeof(int32_t),1,f)!=1){fprintf(stderr,"read fail\n"); return 1;}
        size_t len=0; if(fread(&len,sizeof(size_t),1,f)!=1){fprintf(stderr,"read fail\n"); return 1;}
        if(len>0){ char* buf=malloc(len+1); if(!buf) return 1; if(fread(buf,1,len,f)!=len){free(buf);fprintf(stderr,"read fail\n");return 1;} buf[len]='\0'; printf("[%zu] val=%d key=%s\n", i, val, buf); free(buf); }
        else { printf("[%zu] empty\n", i); }
    }
    fclose(f); return 0; }
