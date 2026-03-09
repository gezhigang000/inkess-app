interface SkillIndicatorProps {
  skillName: string
  className?: string
}

export function SkillIndicator({ skillName, className = '' }: SkillIndicatorProps) {
  return (
    <span
      className={className}
      style={{
        fontSize: 10,
        color: 'var(--accent)',
        background: 'var(--accent-subtle)',
        padding: '1px 6px',
        borderRadius: 4,
        marginLeft: 6,
        whiteSpace: 'nowrap',
      }}
      title={`Active Skill: ${skillName}`}
    >
      {skillName}
    </span>
  )
}
