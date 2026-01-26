    .globl main
main:
    pushq %rbp
    movq %rsp, %rbp
    subq $20, %rsp
    movl $2, -4(%rsp)
    andl $2, -4(%rsp)
    movl $1, -8(%rsp)
    andl $2, -8(%rsp)
    movl -8(%rsp), %r10d
    movl %r10d, -12(%rsp)
    xorl $1, -12(%rsp)
    movl -4(%rsp), %r10d
    movl %r10d, -16(%rsp)
    movl -12(%rsp), %r10d
    orl %r10d, -16(%rsp)
    movl -16(%rsp), %r10d
    movl %r10d, -20(%rsp)
    shl $2, -20(%rsp)
    movl -20(%rsp), %eax
    movq %rbp, %rsp
    popq %rbp
    ret
