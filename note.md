# 编译原理实践：SysYrc

```bash
# 启动 docker 容器，挂载项目目录到容器内
docker run -it --rm -v "D:/wksp/compilers/SysYrc":/root/compiler maxxing/compiler-dev bash

cargo run -- mode hello.c -o hello.s
```