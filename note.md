# 编译原理实践：SysYrc

```bash
# 使用双斜杠 // 开头可以告诉 Shell：“这是一个绝对路径，请不要把它转换成 Windows 路径”
docker run -it --rm -v "D:/wksp/compilers/SysYrc":"//root/compiler" maxxing/compiler-dev autotest -koopa -s lv1 //root/compiler


# 启动 docker 容器，挂载项目目录到容器内
docker run -it --rm -v "D:/wksp/compilers/SysYrc":"//root/compiler" maxxing/compiler-dev bash
autotest -koopa -s lv1 /root/compiler

cargo run -- -koopa hello.c -o hello.s
```

cargo 的版本解析似乎有 bug，如果 rustc 版本不够新，cargo 会错误地解析出与 rustc 不兼容的依赖版本，导致构建失败（也就是求解 rustc 版本约束下的依赖时漏解了）

如果先复制一份 work 的 Cargo.toml 和 Cargo.lock 构建一次，然后再把 Cargo.toml 改成原本的样子，就能成功构建（我猜测是因为从干净基础上构建时会因为漏解而失败，而这个办法使得后一次构建时能使用上次构建时的 cache，从而绕过了漏解的求解过程）
