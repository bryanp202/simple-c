    .globl main
main:
    pushq %rbp
    movq %rsp, %rbp
    subq $8, %rsp
    movl $0, %r11d
    cmpl $0, %r11d
    jne .L0
    movl $0, %r11d
    cmpl $0, %r11d
    movl $0, -4(%rsp)
    sete -4(%rsp)
    cmpl $0, -4(%rsp)
    jne .L0
    movl $0, -8(%rsp)
    jmp .L1
    .L0:
    movl $1, -8(%rsp)
    .L1:
    movl -8(%rsp), %eax
    movq %rbp, %rsp
    popq %rbp
    ret
