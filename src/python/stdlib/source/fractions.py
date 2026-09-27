"""Exact rational arithmetic over the VM's metered arbitrary-precision integers.

Supports integer pairs, fractions, finite floats, and rational/decimal strings.
Decimal objects and the numbers ABC hierarchy are outside this module's surface.
"""

import math as _math

__all__ = ['Fraction']


def _ratio(value):
    if isinstance(value, Fraction):
        return value.numerator, value.denominator
    if isinstance(value, int):
        return value, 1
    if isinstance(value, float):
        if _math.isnan(value):
            raise ValueError('cannot convert NaN to integer ratio')
        if _math.isinf(value):
            raise OverflowError('cannot convert Infinity to integer ratio')
        # Scaling by powers of two is exact, including subnormal floats. The
        # loop takes at most 1074 iterations and each step runs in the VM.
        denominator = 1
        while value != int(value):
            value *= 2
            denominator *= 2
        return int(value), denominator
    raise TypeError('argument should be a string or a Rational instance')


def _string_ratio(text):
    text = text.strip()
    if '/' in text:
        parts = text.split('/')
        if len(parts) != 2:
            raise ValueError('Invalid literal for Fraction: ' + repr(text))
        numerator, denominator = parts[0].strip(), parts[1].strip()
        # int also accepts surrounding whitespace and underscores.
        if not numerator or not denominator or denominator[0] in '+-':
            raise ValueError('Invalid literal for Fraction: ' + repr(text))
        return int(numerator), int(denominator)
    sign = 1
    if (text.startswith('+') or text.startswith('-')):
        if text[0] == '-':
            sign = -1
        text = text[1:]
    exponent = 0
    parts = text.lower().split('e')
    if len(parts) > 2:
        raise ValueError('Invalid literal for Fraction: ' + repr(text))
    if len(parts) == 2:
        exponent = int(parts[1])
    parts = parts[0].split('.')
    if len(parts) > 2:
        raise ValueError('Invalid literal for Fraction: ' + repr(text))
    whole = parts[0]
    fractional = parts[1] if len(parts) == 2 else ''
    # Validate each digit run before removing separators, to reject 1__2.
    for digits in (whole, fractional):
        if digits:
            int(digits)
            if (digits.startswith('+') or digits.startswith('-')) or any(c in ' \t\n\r\v\f' for c in digits):
                raise ValueError('Invalid literal for Fraction: ' + repr(text))
    digits = whole.replace('_', '') + fractional.replace('_', '')
    if not digits or not digits.isdigit():
        raise ValueError('Invalid literal for Fraction: ' + repr(text))
    scale = len(fractional.replace('_', '')) - exponent
    numerator = sign * int(digits)
    if scale < 0:
        return numerator * 10 ** (-scale), 1
    return numerator, 10 ** scale


class Fraction:
    def __init__(self, numerator=0, denominator=None):
        if denominator is None:
            if isinstance(numerator, str):
                numerator, denominator = _string_ratio(numerator)
            else:
                numerator, denominator = _ratio(numerator)
        else:
            if not isinstance(numerator, (int, Fraction)) or not isinstance(denominator, (int, Fraction)):
                raise TypeError('both arguments should be Rational instances')
            n, nd = _ratio(numerator)
            d, dd = _ratio(denominator)
            numerator, denominator = n * dd, nd * d
        if denominator == 0:
            raise ZeroDivisionError('Fraction(%s, 0)' % numerator)
        divisor = _math.gcd(numerator, denominator)
        if denominator < 0:
            divisor = -divisor
        self._numerator = numerator // divisor
        self._denominator = denominator // divisor

    @property
    def numerator(self):
        return self._numerator

    @property
    def denominator(self):
        return self._denominator

    def as_integer_ratio(self):
        return self.numerator, self.denominator

    def is_integer(self):
        return self.denominator == 1

    def __str__(self):
        if self.denominator == 1:
            return str(self.numerator)
        return str(self.numerator) + '/' + str(self.denominator)

    def __repr__(self):
        return 'Fraction(%s, %s)' % (self.numerator, self.denominator)

    def __bool__(self):
        return self.numerator != 0

    def __int__(self):
        result = abs(self.numerator) // self.denominator
        return -result if self.numerator < 0 else result

    def __float__(self):
        return self.numerator / self.denominator

    def __neg__(self):
        return Fraction(-self.numerator, self.denominator)

    def __pos__(self):
        return Fraction(self)

    def __abs__(self):
        return Fraction(abs(self.numerator), self.denominator)

    def __add__(self, other):
        if isinstance(other, (float, complex)):
            return float(self) + other
        n, d = _ratio(other)
        return Fraction(self.numerator * d + n * self.denominator, self.denominator * d)

    def __radd__(self, other):
        return self + other

    def __sub__(self, other):
        if isinstance(other, (float, complex)):
            return float(self) - other
        n, d = _ratio(other)
        return Fraction(self.numerator * d - n * self.denominator, self.denominator * d)

    def __rsub__(self, other):
        if isinstance(other, (float, complex)):
            return other - float(self)
        return Fraction(other) - self

    def __mul__(self, other):
        if isinstance(other, (float, complex)):
            return float(self) * other
        n, d = _ratio(other)
        return Fraction(self.numerator * n, self.denominator * d)

    def __rmul__(self, other):
        return self * other

    def __truediv__(self, other):
        if isinstance(other, (float, complex)):
            return float(self) / other
        n, d = _ratio(other)
        return Fraction(self.numerator * d, self.denominator * n)

    def __rtruediv__(self, other):
        if isinstance(other, (float, complex)):
            return other / float(self)
        return Fraction(other) / self

    def __floordiv__(self, other):
        if isinstance(other, float):
            return float(self) // other
        n, d = _ratio(other)
        return (self.numerator * d) // (self.denominator * n)

    def __rfloordiv__(self, other):
        if isinstance(other, float):
            return other // float(self)
        return Fraction(other) // self

    def __mod__(self, other):
        return self - (self // other) * other

    def __rmod__(self, other):
        return other - (other // self) * self

    def __pow__(self, exponent):
        if isinstance(exponent, Fraction) and exponent.denominator == 1:
            exponent = exponent.numerator
        if isinstance(exponent, int):
            if exponent >= 0:
                return Fraction(self.numerator ** exponent, self.denominator ** exponent)
            return Fraction(self.denominator ** (-exponent), self.numerator ** (-exponent))
        return float(self) ** float(exponent)

    def __eq__(self, other):
        if isinstance(other, float) and not _math.isfinite(other):
            return False
        if isinstance(other, complex):
            return other.imag == 0 and self == other.real
        if not isinstance(other, (int, float, Fraction)):
            return False
        n, d = _ratio(other)
        return self.numerator * d == n * self.denominator

    def __ne__(self, other):
        return not self == other

    def __lt__(self, other):
        if isinstance(other, float) and not _math.isfinite(other):
            return other == _math.inf
        n, d = _ratio(other)
        return self.numerator * d < n * self.denominator

    def __le__(self, other):
        return self == other or self < other

    def __gt__(self, other):
        if isinstance(other, float) and not _math.isfinite(other):
            return other == -_math.inf
        n, d = _ratio(other)
        return self.numerator * d > n * self.denominator

    def __ge__(self, other):
        return self == other or self > other

    def limit_denominator(self, max_denominator=1000000):
        if not isinstance(max_denominator, int):
            raise TypeError('max_denominator must be an integer')
        if max_denominator < 1:
            raise ValueError('max_denominator should be at least 1')
        if self.denominator <= max_denominator:
            return Fraction(self)
        # Continued fractions produce the two nearest bounding rationals.
        p0, q0, p1, q1 = 0, 1, 1, 0
        n, d = self.numerator, self.denominator
        while True:
            a = n // d
            q2 = q0 + a * q1
            if q2 > max_denominator:
                break
            p0, q0, p1, q1 = p1, q1, p0 + a * p1, q2
            n, d = d, n - a * d
        k = (max_denominator - q0) // q1
        bound1 = Fraction(p0 + k * p1, q0 + k * q1)
        bound2 = Fraction(p1, q1)
        if abs(bound2 - self) <= abs(bound1 - self):
            return bound2
        return bound1
