    .globl main
main:
    pushq %rbp
    movq %rsp, %rbp
    subq $80, %rsp
    movl $2, %r11d
    cmpl $2, %r11d
    movl $0, -4(%rsp)
    setne -4(%rsp)
    movl $2, %r11d
    cmpl $2, %r11d
    movl $0, -8(%rsp)
    setne -8(%rsp)
    cmpl $0, -8(%rsp)
    movl $0, -12(%rsp)
    sete -12(%rsp)
    movl -12(%rsp), %r10d
    movl %r10d, -16(%rsp)
    movl -16(%rsp), %r11d
    imull $2, %r11d
    movl %r11d, -16(%rsp)
    movl $2, -20(%rsp)
    movl -16(%rsp), %r10d
    addl %r10d, -20(%rsp)
    movl $12312, -24(%rsp)
    notl -24(%rsp)
    movl -20(%rsp), %r10d
    movl %r10d, -28(%rsp)
    movl -24(%rsp), %r10d
    addl %r10d, -28(%rsp)
    movl $423, -32(%rsp)
    movl -32(%rsp), %r11d
    imull $43, %r11d
    movl %r11d, -32(%rsp)
    movl -28(%rsp), %r10d
    movl %r10d, -36(%rsp)
    movl -32(%rsp), %r10d
    subl %r10d, -36(%rsp)
    movl $2, %r11d
    cmpl $2, %r11d
    movl $0, -40(%rsp)
    setne -40(%rsp)
    cmpl $0, -40(%rsp)
    jne .L0
    movl $3, -44(%rsp)
    subl $1, -44(%rsp)
    cmpl $2, -44(%rsp)
    movl $0, -48(%rsp)
    sete -48(%rsp)
    cmpl $0, -48(%rsp)
    jne .L0
    movl $0, -52(%rsp)
    jmp .L1
.L0:
    movl $1, -52(%rsp)
.L1:
    movl $3, -56(%rsp)
    subl $5, -56(%rsp)
    movl $4, -60(%rsp)
    sar $1, -60(%rsp)
    movl -56(%rsp), %r10d
    cmpl %r10d, -60(%rsp)
    movl $0, -64(%rsp)
    setle -64(%rsp)
    movl -52(%rsp), %r10d
    movl %r10d, -68(%rsp)
    movl -64(%rsp), %ecx
    shl %cl, -68(%rsp)
    movl $10, %eax
    cdq
    idivl -68(%rsp)
    movl %eax, -72(%rsp)
    movl -36(%rsp), %r10d
    movl %r10d, -76(%rsp)
    movl -72(%rsp), %r10d
    addl %r10d, -76(%rsp)
    movl -4(%rsp), %r10d
    movl %r10d, -80(%rsp)
    movl -76(%rsp), %r10d
    andl %r10d, -80(%rsp)
    movl -80(%rsp), %eax
    movq %rbp, %rsp
    popq %rbp
    ret
