using System;
using System.ComponentModel;
using System.Globalization;

namespace LspfAnalysis
{
    public sealed class OptionBooleanConverter : BooleanConverter
    {
        public override object ConvertTo(ITypeDescriptorContext context, CultureInfo culture, object value, Type destinationType)
        {
            if (destinationType == typeof(string) && value is bool enabled && culture != CultureInfo.InvariantCulture)
                return enabled ? Labels.Text("True", "是") : Labels.Text("False", "否");
            return base.ConvertTo(context, culture, value, destinationType);
        }

        public override object ConvertFrom(ITypeDescriptorContext context, CultureInfo culture, object value)
        {
            if (value is string text && text == "是") return true;
            if (value is string other && other == "否") return false;
            return base.ConvertFrom(context, culture, value);
        }
    }

    public sealed class OptionTraceConverter : EnumConverter
    {
        public OptionTraceConverter() : base(typeof(ServerTrace)) { }
        public override object ConvertTo(ITypeDescriptorContext context, CultureInfo culture, object value, Type destinationType)
        {
            if (destinationType == typeof(string) && value is ServerTrace trace && culture != CultureInfo.InvariantCulture)
            {
                switch (trace)
                {
                    case ServerTrace.Off: return Labels.Text("Off", "关闭");
                    case ServerTrace.Messages: return Labels.Text("Messages", "消息");
                    case ServerTrace.Verbose: return Labels.Text("Verbose", "详细");
                }
            }
            return base.ConvertTo(context, culture, value, destinationType);
        }

        public override object ConvertFrom(ITypeDescriptorContext context, CultureInfo culture, object value)
        {
            return (value as string) switch
            {
                "关闭" => ServerTrace.Off,
                "消息" => ServerTrace.Messages,
                "详细" => ServerTrace.Verbose,
                _ => base.ConvertFrom(context, culture, value),
            };
        }
    }

    [AttributeUsage(AttributeTargets.Property)]
    internal sealed class OptionCategoryAttribute : CategoryAttribute
    {
        private readonly string chinese;
        internal OptionCategoryAttribute(string english, string chinese) : base(english) => this.chinese = chinese;
        protected override string GetLocalizedString(string value) => Labels.Text(value, chinese);
    }

    [AttributeUsage(AttributeTargets.Property)]
    internal sealed class OptionNameAttribute : DisplayNameAttribute
    {
        private readonly string chinese;
        internal OptionNameAttribute(string english, string chinese) : base(english) => this.chinese = chinese;
        public override string DisplayName => Labels.Text(base.DisplayName, chinese);
    }

    [AttributeUsage(AttributeTargets.Property)]
    internal sealed class OptionDescriptionAttribute : DescriptionAttribute
    {
        private readonly string chinese;
        internal OptionDescriptionAttribute(string english, string chinese) : base(english) => this.chinese = chinese;
        public override string Description => Labels.Text(base.Description, chinese);
    }
}
