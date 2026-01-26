    .globl main
main:
    pushq %rbp
    movq %rsp, %rbp
    subq $16, %rsp
    movl $2, %r11d
    cmpl $2, %r11d
    movl $0, -4(%rsp)
    setne -4(%rsp)
    cmpl $0, -4(%rsp)
    jne .L0
    movl $2, %r11d
    cmpl $2, %r11d
    movl $0, -8(%rsp)
    setne -8(%rsp)
    cmpl $0, -8(%rsp)
    movl $0, -12(%rsp)
    sete -12(%rsp)
    cmpl $0, -12(%rsp)
    jne .L0
    movl $0, -16(%rsp)
    jmp .L1
.L0:
    movl $1, -16(%rsp)
.L1:
    movl -16(%rsp), %eax
    movq %rbp, %rsp
    popq %rbp
    ret
