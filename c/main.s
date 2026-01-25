    .globl main
main:
    pushq %rbp
    movq %rsp, %rbp
    subq $28, %rsp
    movl $2, -4(%rsp)
    movl -4(%rsp), %r11d
    imull $423, %r11d
    movl %r11d, -4(%rsp)
    movl -4(%rsp), %eax
    cdq
    movl $3, %r10d
    idivl %r10d
    movl %eax, -8(%rsp)
    movl $2, -12(%rsp)
    addl $3, -12(%rsp)
    movl -12(%rsp), %eax
    cdq
    movl $2, %r10d
    idivl %r10d
    movl %eax, -16(%rsp)
    movl $5, -20(%rsp)
    movl -20(%rsp), %r11d
    imull -16(%rsp), %r11d
    movl %r11d, -20(%rsp)
    movl -20(%rsp), %eax
    cdq
    movl $2, %r10d
    idivl %r10d
    movl %eax, -24(%rsp)
    movl -8(%rsp), %r10d
    movl %r10d, -28(%rsp)
    movl -24(%rsp), %r10d
    addl %r10d, -28(%rsp)
    movl -28(%rsp), %eax
    movq %rbp, %rsp
    popq %rbp
    ret
