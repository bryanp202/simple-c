    .globl main
main:
    pushq %rbp
    movq %rsp, %rbp
    subq $12, %rsp
    movl $2, -4(%rsp)
    movl -4(%rsp), %r11d
    imull $2, %r11d
    movl %r11d, -4(%rsp)
    movl $2, -8(%rsp)
    movl -4(%rsp), %r10d
    addl %r10d, -8(%rsp)
    movl -8(%rsp), %r10d
    movl %r10d, -12(%rsp)
    addl $2, -12(%rsp)
    movl -12(%rsp), %eax
    movq %rbp, %rsp
    popq %rbp
    ret
