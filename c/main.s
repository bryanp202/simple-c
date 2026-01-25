    .globl main
main:
    pushq %rbp
    movq %rsp, %rbp
    subq $40, %rsp
    movl $128, -4(%rsp)
    movl -4(%rsp), %r11d
    imull $2, %r11d
    movl %r11d, -4(%rsp)
    movl $2, %eax
    cdq
    movl $1, %r10d
    idivl %r10d
    movl %eax, -8(%rsp)
    movl $1, -12(%rsp)
    movl -12(%rsp), %r11d
    imull $2, %r11d
    movl %r11d, -12(%rsp)
    movl -8(%rsp), %r10d
    movl %r10d, -16(%rsp)
    movl -12(%rsp), %r10d
    addl %r10d, -16(%rsp)
    movl -16(%rsp), %r10d
    movl %r10d, -20(%rsp)
    addl $1, -20(%rsp)
    movl -4(%rsp), %r10d
    movl %r10d, -24(%rsp)
    movl -20(%rsp), %ecx
    shr %cl, -24(%rsp)
    movl $10, -28(%rsp)
    addl $1, -28(%rsp)
    movl -24(%rsp), %r10d
    movl %r10d, -32(%rsp)
    movl -28(%rsp), %ecx
    shl %cl, -32(%rsp)
    movl $1, -36(%rsp)
    negl -36(%rsp)
    movl -32(%rsp), %r10d
    movl %r10d, -40(%rsp)
    movl -36(%rsp), %ecx
    shr %cl, -40(%rsp)
    movl -40(%rsp), %eax
    movq %rbp, %rsp
    popq %rbp
    ret
