    .globl main
main:
    pushq %rbp
    movq %rsp, %rbp
    subq $16, %rsp
    movl $2, -4(%rsp)
    and $2, -4(%rsp)
    movl $1, -8(%rsp)
    and $1, -8(%rsp)
    movl -8(%rsp), %r10d
    movl %r10d, -12(%rsp)
    xor $1, -12(%rsp)
    movl -4(%rsp), %r10d
    movl %r10d, -16(%rsp)
    movl -12(%rsp), %r10d
    or %r10d, -16(%rsp)
    movl -16(%rsp), %eax
    movq %rbp, %rsp
    popq %rbp
    ret
